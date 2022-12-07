use std::{
    collections::HashMap,
    env,
    io,
    num::ParseIntError,
    sync::Arc,
    time::Duration,
};

use async_rustls::{
    rustls,
    webpki,
};
use muxado::heartbeat::HeartbeatConfig;
use thiserror::Error;
use tokio::sync::{
    mpsc::{
        channel,
        Sender,
    },
    Mutex,
    RwLock,
};
use tokio_util::compat::{
    FuturesAsyncReadCompatExt,
    TokioAsyncReadCompatExt,
};
use tracing::warn;

use crate::{
    config::TunnelConfig,
    internals::{
        proto::{
            AuthExtra,
            AuthResp,
        },
        raw_session::{
            AcceptError as RawAcceptError,
            IncomingStreams,
            RawSession,
            RpcClient,
            RpcError,
            StartSessionError,
        },
    },
    AcceptError,
    Conn,
    Tunnel,
};

const CERT_BYTES: &[u8] = include_bytes!("../assets/ngrok.ca.crt");
const NOT_IMPLEMENTED: &str = "the agent has not defined a callback for this operation";

type TunnelConns = HashMap<String, Sender<Result<Conn, AcceptError>>>;

/// An ngrok session.
#[derive(Clone)]
pub struct Session {
    #[allow(dead_code)]
    authresp: AuthResp,
    client: Arc<Mutex<RpcClient>>,
    tunnels: Arc<RwLock<TunnelConns>>,
}

/// The builder for an ngrok [Session].
#[derive(Clone)]
pub struct SessionBuilder {
    authtoken: Option<String>,
    metadata: Option<String>,
    heartbeat_interval: Option<Duration>,
    heartbeat_tolerance: Option<Duration>,
    server_addr: (String, u16),
    tls_config: rustls::ClientConfig,
}

/// Errors arising at [SessionBuilder::connect] time.
#[derive(Error, Debug)]
pub enum ConnectError {
    /// The builder specified an invalid heartbeat interval.
    ///
    /// This is most likely caused a [Duration] that's outside of the [i64::MAX]
    /// nanosecond range.
    #[error("invalid heartbeat interval: {0}")]
    InvalidHeartbeatInterval(u128),
    /// The builder specified an invalid heartbeat tolerance.
    ///
    /// This is most likely caused a [Duration] that's outside of the [i64::MAX]
    /// nanosecond range.
    #[error("invalid heartbeat tolerance: {0}")]
    InvalidHeartbeatTolerance(u128),
    /// An error occurred when establishing a TCP connection to the ngrok
    /// server.
    #[error("failed to establish tcp connection")]
    Tcp(io::Error),
    /// A TLS handshake error occurred.
    ///
    /// This is usually a certificate validation issue, or an attempt to connect
    /// to something that doesn't actually speak TLS.
    #[error("tls handshake error")]
    Tls(io::Error),
    /// An error occurred when starting the ngrok session.
    ///
    /// This might occur when there's a protocol mismatch interfering with the
    /// heartbeat routine.
    #[error("failed to start ngrok session")]
    Start(StartSessionError),
    /// An error occurred when attempting to authenticate.
    #[error("authentication failure")]
    Auth(RpcError),
}

impl Default for SessionBuilder {
    fn default() -> Self {
        let mut root_store = rustls::RootCertStore::empty();
        let mut cert_pem = io::Cursor::new(CERT_BYTES);
        root_store
            .add_pem_file(&mut cert_pem)
            .expect("a valid ngrok root cert");

        let mut tls_config = rustls::ClientConfig::new();
        tls_config.root_store = root_store;

        SessionBuilder {
            authtoken: None,
            metadata: None,
            heartbeat_interval: None,
            heartbeat_tolerance: None,
            server_addr: ("tunnel.ngrok.com".into(), 443),
            tls_config,
        }
    }
}

/// An invalid server address was provided.
#[derive(Debug, Error)]
#[error("invalid server address")]
pub struct InvalidAddrError(#[source] ParseIntError);

impl SessionBuilder {
    /// Authenticate the ngrok session with the given authtoken.
    pub fn with_authtoken(&mut self, authtoken: impl Into<String>) -> &mut Self {
        self.authtoken = Some(authtoken.into());
        self
    }

    /// Authenticate using the authtoken in the `NGROK_AUTHTOKEN` environment
    /// variable.
    pub fn with_authtoken_from_env(&mut self) -> &mut Self {
        self.authtoken = env::var("NGROK_AUTHTOKEN").ok();
        self
    }

    /// Set the heartbeat interval for the session.
    /// This value determines how often we send application level
    /// heartbeats to the server go check connection liveness.
    pub fn with_heartbeat_interval(&mut self, heartbeat_interval: Duration) -> &mut Self {
        self.heartbeat_interval = Some(heartbeat_interval);
        self
    }

    /// Set the heartbeat tolerance for the session.
    /// If the session's heartbeats are outside of their interval by this duration,
    /// the server will assume the session is dead and close it.
    pub fn with_heartbeat_tolerance(&mut self, heartbeat_tolerance: Duration) -> &mut Self {
        self.heartbeat_tolerance = Some(heartbeat_tolerance);
        self
    }

    /// Use the provided opaque metadata string for this session.
    /// Viewable from the ngrok dashboard or API.
    pub fn with_metadata(&mut self, metadata: impl Into<String>) -> &mut Self {
        self.metadata = Some(metadata.into());
        self
    }

    /// Connect to the provided ngrok server address.
    pub fn with_server_addr(
        &mut self,
        addr: impl AsRef<str>,
    ) -> Result<&mut Self, InvalidAddrError> {
        let addr = addr.as_ref();
        let mut split = addr.split(':');
        let host = split.next().unwrap().into();
        let port = split
            .next()
            .map(str::parse::<u16>)
            .transpose()
            .map_err(InvalidAddrError)?;
        self.server_addr = (host, port.unwrap_or(443));
        Ok(self)
    }

    /// Use the provided tls config when connecting to the ngrok server.
    pub fn with_tls_config(&mut self, config: rustls::ClientConfig) -> &mut Self {
        self.tls_config = config;
        self
    }

    /// Attempt to establish an ngrok session using the current configuration.
    pub async fn connect(&self) -> Result<Session, ConnectError> {
        let conn = tokio::net::TcpStream::connect(&self.server_addr)
            .await
            .map_err(ConnectError::Tcp)?
            .compat();

        let tls_conn = async_rustls::TlsConnector::from(Arc::new(self.tls_config.clone()))
            .connect(
                webpki::DNSNameRef::try_from_ascii(self.server_addr.0.as_bytes()).unwrap(),
                conn,
            )
            .await
            .map_err(ConnectError::Tls)?;

        let mut heartbeat_config = HeartbeatConfig::<fn(Duration)>::default();
        if let Some(interval) = self.heartbeat_interval {
            heartbeat_config.interval = interval;
        }
        if let Some(tolerance) = self.heartbeat_tolerance {
            heartbeat_config.tolerance = tolerance;
        }
        // convert these while we have ownership
        let interval_nanos = heartbeat_config.interval.as_nanos();
        let heartbeat_interval = i64::try_from(interval_nanos)
            .map_err(|_| ConnectError::InvalidHeartbeatInterval(interval_nanos))?;
        let tolerance_nanos = heartbeat_config.interval.as_nanos();
        let heartbeat_tolerance = i64::try_from(tolerance_nanos)
            .map_err(|_| ConnectError::InvalidHeartbeatTolerance(tolerance_nanos))?;

        let mut raw = RawSession::start(tls_conn.compat(), heartbeat_config)
            .await
            .map_err(ConnectError::Start)?;

        // list of possibilities: https://doc.rust-lang.org/std/env/consts/constant.OS.html
        let os = match env::consts::OS {
            "macos" => "darwin",
            _ => env::consts::OS,
        };

        let resp = raw
            .auth(
                "",
                AuthExtra {
                    version: env!("CARGO_PKG_VERSION").into(),
                    auth_token: self.authtoken.clone().unwrap_or_default(),
                    metadata: self.metadata.clone().unwrap_or_default(),
                    os: os.into(),
                    arch: std::env::consts::ARCH.into(),
                    heartbeat_interval,
                    heartbeat_tolerance,
                    restart_unsupported_error: Some(NOT_IMPLEMENTED.into()),
                    stop_unsupported_error: Some(NOT_IMPLEMENTED.into()),
                    update_unsupported_error: Some(NOT_IMPLEMENTED.into()),
                    client_type: "library/official/rust".into(),
                    ..Default::default()
                },
            )
            .await
            .map_err(ConnectError::Auth)?;

        let (client, incoming) = raw.split();

        let tunnels: Arc<RwLock<TunnelConns>> = Default::default();

        tokio::spawn(accept_incoming(incoming, tunnels.clone()));

        Ok(Session {
            authresp: resp,
            client: Arc::new(Mutex::new(client)),
            tunnels,
        })
    }
}

impl Session {
    /// Create a new [SessionBuilder] to configure a new ngrok session.
    pub fn builder() -> SessionBuilder {
        SessionBuilder::default()
    }

    /// Start a new tunnel in this session.
    pub async fn start_tunnel<C>(&self, tunnel_cfg: C) -> Result<Tunnel, RpcError>
    where
        C: TunnelConfig,
    {
        let mut client = self.client.lock().await;

        // let tunnelCfg: dyn TunnelConfig = TunnelConfig(opts);
        let (tx, rx) = channel(64);

        // non-labeled tunnel
        if tunnel_cfg.proto() != "" {
            let resp = client
                .listen(
                    tunnel_cfg.proto(),
                    tunnel_cfg.opts().unwrap(), // this is crate-defined, and must exist if proto is non-empty
                    tunnel_cfg.extra(),
                    "",
                    tunnel_cfg.forwards_to(),
                )
                .await?;

            let mut tunnels = self.tunnels.write().await;
            tunnels.insert(resp.client_id.clone(), tx);

            return Ok(Tunnel {
                id: resp.client_id,
                proto: resp.proto,
                url: resp.url,
                opts: resp.bind_opts,
                token: resp.extra.token,
                bind_extra: tunnel_cfg.extra(),
                labels: HashMap::new(),
                forwards_to: tunnel_cfg.forwards_to(),
                session: self.clone(),
                incoming: rx,
            });
        }

        // labeled tunnel
        let resp = client
            .listen_label(
                tunnel_cfg.labels(),
                tunnel_cfg.extra().metadata,
                tunnel_cfg.forwards_to(),
            )
            .await?;

        let mut tunnels = self.tunnels.write().await;
        tunnels.insert(resp.id.clone(), tx);

        Ok(Tunnel {
            id: resp.id,
            proto: Default::default(),
            url: Default::default(),
            opts: Default::default(),
            token: Default::default(),
            bind_extra: tunnel_cfg.extra(),
            labels: tunnel_cfg.labels(),
            forwards_to: tunnel_cfg.forwards_to(),
            session: self.clone(),
            incoming: rx,
        })
    }

    /// Close a tunnel with the given ID.
    pub async fn close_tunnel(&self, id: impl AsRef<str>) -> Result<(), RpcError> {
        let id = id.as_ref();
        self.client.lock().await.unlisten(id).await?;
        self.tunnels.write().await.remove(id);
        Ok(())
    }
}

async fn accept_incoming(mut incoming: IncomingStreams, tunnels: Arc<RwLock<TunnelConns>>) {
    let error: AcceptError = loop {
        let conn = match incoming.accept().await {
            Ok(conn) => conn,
            // Assume if we got a muxado error, the session is borked. Break and
            // propagate the error to all of the tunnels out in the wild.
            Err(RawAcceptError::Transport(error)) => break error,
            // The other errors are either a bad header or an unrecognized
            // stream type. They're non-fatal, but could signal a protocol
            // mismatch.
            Err(error) => {
                warn!(?error, "protocol error when accepting tunnel connection");
                continue;
            }
        };
        let id = conn.header.id.clone();
        let remote_addr = conn.header.client_addr.parse().unwrap_or_else(|error| {
            warn!(
                client_addr = conn.header.client_addr,
                %error,
                "invalid remote addr for tunnel connection",
            );
            "0.0.0.0:0".parse().unwrap()
        });
        let guard = tunnels.read().await;
        let res = if let Some(ch) = guard.get(&id) {
            ch.send(Ok(Conn {
                remote_addr,
                stream: conn.stream,
            }))
            .await
        } else {
            Ok(())
        };
        drop(guard);
        if res.is_err() {
            RwLock::write(&tunnels).await.remove(&id);
        }
    }
    .into();
    for (_id, ch) in tunnels.write().await.drain() {
        let _ = ch.send(Err(error)).await;
    }
}
