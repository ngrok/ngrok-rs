use std::sync::Arc;

use napi::bindgen_prelude::*;
use tokio::sync::Mutex;
use tracing::debug;
use tracing_subscriber::{
    self,
    fmt::format::FmtSpan,
};

use crate::{
    config::{
        TcpTunnelBuilder,
        TunnelBuilder,
    },
    session::SessionBuilder,
    tunnel::TcpTunnel,
    tunnel_ext::TunnelExt,
    Session,
    Tunnel,
};

#[napi(js_name = "SessionBuilder")]
#[allow(dead_code)]
pub struct JsSessionBuilder {
    raw_builder: SessionBuilder,
}

#[napi]
#[cfg_attr(feature = "cargo-clippy", allow(clippy::new_without_default))]
impl JsSessionBuilder {
    #[napi(constructor)]
    pub fn new() -> Self {
        tracing_subscriber::fmt()
            .pretty()
            .with_span_events(FmtSpan::ENTER)
            .with_env_filter(std::env::var("RUST_LOG").unwrap_or_default())
            .init();

        JsSessionBuilder {
            raw_builder: Session::builder(),
        }
    }

    #[napi]
    pub fn authtoken_from_env(&mut self) -> &Self {
        // can't put lifetimes or generics on napi structs, which limits our options.
        // there is a Reference which can force static lifetime, but haven't figured
        // out a way to make this actually helpful. so send in the clones.
        // https://napi.rs/docs/concepts/reference
        self.raw_builder = self.raw_builder.clone().authtoken_from_env();
        self
    }

    #[napi]
    pub fn metadata(&mut self, metadata: String) -> &Self {
        self.raw_builder = self.raw_builder.clone().metadata(metadata);
        self
    }

    #[napi]
    pub async fn connect(&self) -> Result<JsSession> {
        self.raw_builder
            .connect()
            .await
            .map(|s| JsSession { raw_session: s })
            .map_err(|e| {
                Error::new(
                    Status::GenericFailure,
                    format!("failed to connect session, {e}"),
                )
            })
    }
}

#[napi(js_name = "Session", custom_finalize)]
pub struct JsSession {
    #[allow(dead_code)]
    raw_session: Session,
}

#[napi]
impl JsSession {
    #[napi(constructor)]
    pub fn unused() -> Result<Self> {
        Err(Error::new(
            Status::GenericFailure,
            "cannot instantiate".to_string(),
        ))
    }

    #[napi]
    pub fn tcp_endpoint(&self) -> JsTcpEndpoint {
        JsTcpEndpoint {
            tcp_endpoint: self.raw_session.tcp_endpoint(),
        }
    }
}

impl ObjectFinalize for JsSession {
    fn finalize(self, mut _env: Env) -> Result<()> {
        debug!("JsSession finalize");
        Ok(())
    }
}

#[napi(js_name = "TcpEndpoint", custom_finalize)]
pub struct JsTcpEndpoint {
    tcp_endpoint: TcpTunnelBuilder,
}

#[napi]
impl JsTcpEndpoint {
    #[napi(constructor)]
    pub fn unused() -> Result<Self> {
        Err(Error::new(
            Status::GenericFailure,
            "cannot instantiate".to_string(),
        ))
    }

    #[napi]
    pub fn metadata(&mut self, metadata: String) -> &Self {
        self.tcp_endpoint = self.tcp_endpoint.clone().metadata(metadata);
        self
    }

    #[napi]
    pub fn remote_addr(&mut self, remote_addr: String) -> &Self {
        self.tcp_endpoint = self.tcp_endpoint.clone().remote_addr(remote_addr);
        self
    }

    #[napi]
    pub async fn listen(&self) -> Result<JsTunnel> {
        self.tcp_endpoint
            .listen()
            .await
            .map(JsTunnel::new)
            .map_err(|e| {
                Error::new(
                    Status::GenericFailure,
                    format!("failed to start tunnel: {e}"),
                )
            })
    }
}

impl ObjectFinalize for JsTcpEndpoint {
    fn finalize(self, mut _env: Env) -> Result<()> {
        debug!("JsTcpEndpoint finalize");
        Ok(())
    }
}

#[napi(js_name = "Tunnel", custom_finalize)]
pub struct JsTunnel {
    #[allow(dead_code)]
    id: String,
    #[allow(dead_code)]
    url: String,
    #[allow(dead_code)]
    raw_tunnel: Arc<Mutex<TcpTunnel>>, // TunnelExt is not an object safe trait, so storing real type
}

#[napi]
impl JsTunnel {
    #[napi(constructor)]
    pub fn unused() -> Result<Self> {
        Err(Error::new(
            Status::GenericFailure,
            "cannot instantiate".to_string(),
        ))
    }

    fn new(raw_tunnel: TcpTunnel) -> Self {
        JsTunnel {
            id: raw_tunnel.id().to_string(),
            url: raw_tunnel.inner.url().to_string(),
            raw_tunnel: Arc::new(Mutex::new(raw_tunnel)),
        }
    }

    #[napi]
    pub fn get_id(&self) -> String {
        self.id.clone()
    }

    #[napi]
    pub fn get_url(&self) -> String {
        self.url.clone()
    }

    #[napi]
    pub async fn forward_tcp(&self, addr: String) -> Result<()> {
        self.raw_tunnel
            .lock()
            .await
            .forward_tcp(addr)
            .await
            .map_err(|e| Error::new(Status::GenericFailure, format!("cannot forward tcp: {e}")))
    }

    #[napi]
    pub async fn forward_unix(&self, addr: String) -> Result<()> {
        self.raw_tunnel
            .lock()
            .await
            .forward_unix(addr)
            .await
            .map_err(|e| Error::new(Status::GenericFailure, format!("cannot forward unix: {e}")))
    }
}

impl ObjectFinalize for JsTunnel {
    fn finalize(self, mut _env: Env) -> Result<()> {
        debug!("JsTunnel finalize");
        Ok(())
    }
}
