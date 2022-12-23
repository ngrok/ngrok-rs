use std::sync::Arc;

use napi::bindgen_prelude::*;
use tokio::sync::Mutex;

use crate::{
    config::TunnelBuilder,
    Session,
    Tunnel,
    tunnel_ext::TunnelExt, tunnel::TcpTunnel,
};

#[napi]
#[allow(dead_code)]
async fn session() -> Result<JsSession> {
    Session::builder()
        .authtoken_from_env()
        .metadata("Online in One Line")
        .connect()
        .await
        .map(|s| JsSession { raw_session: s })
        .map_err(|e| {
            Error::new(
                Status::GenericFailure,
                format!("failed to read file, {}", e),
            )
        })
}

#[napi(js_name = "Session")]
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
            format!("cannot instantiate"),
        ))
    }

    #[napi]
    pub async fn hi(&self) -> Result<String> {
        Ok("hi".to_string())
    }

    #[napi]
    pub async fn start_tunnel(&self) -> Result<JsTunnel> {
        self.raw_session
            .tcp_endpoint()
            .listen()
            .await
            .map(JsTunnel::new)
            .map_err(|e| {
                Error::new(
                    Status::GenericFailure,
                    format!("failed to start tunnel: {}", e),
                )
            })
    }
}


#[napi(js_name = "Tunnel")]
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
            format!("cannot instantiate"),
        ))
    }

    fn new(raw_tunnel: TcpTunnel) -> Self
    {
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
    pub async fn forward_http(&self, addr: String) -> Result<()> {
        self.raw_tunnel.lock().await.forward_http(addr).await
            .map_err(|e| Error::new(
                Status::GenericFailure,
                format!("cannot forward http: {}", e),
            ))
    }
}