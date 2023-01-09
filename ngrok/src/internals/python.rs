use std::{
    collections::HashMap,
    sync::Arc,
};

use blocking::block_on;
use bytes::BytesMut;
use futures::TryStreamExt;
use pyo3::{
    exceptions::PyValueError,
    pyclass,
    pyfunction,
    pymethods,
    pymodule,
    types::{
        PyByteArray,
        PyDict,
        PyModule,
    },
    wrap_pyfunction,
    PyAny,
    PyErr,
    PyResult,
    Python,
};
use tokio::{
    io::{
        self,
        AsyncReadExt,
        AsyncWriteExt,
        ReadHalf,
        WriteHalf,
    },
    sync::Mutex,
};

use crate::{
    config::TunnelBuilder,
    prelude::TunnelExt,
    tunnel::TcpTunnel,
    Conn as RawConn,
    Session as RawSession,
};

#[pyclass]
#[derive(Clone)]
struct Session {
    raw_session: RawSession,
}

impl Session {
    fn new(raw_session: RawSession) -> Self {
        Session { raw_session }
    }
}

#[pymethods]
impl Session {
    fn __str__(&self) -> String {
        "ngrok_session".to_string()
    }

    #[args(py_kwargs = "**")]
    #[allow(clippy::needless_lifetimes)] // clippy has its limits, these are required
    fn start_tunnel<'a>(&self, py: Python<'a>, py_kwargs: Option<&PyDict>) -> PyResult<&'a PyAny> {
        let s: Session = self.clone();
        let map = py_kwargs.map(|k| k.extract().unwrap());
        pyo3_asyncio::tokio::future_into_py(py, async move { internal_start_tunnel(&s, map).await })
    }
}

async fn internal_connect(kwargs: Option<HashMap<String, String>>) -> Result<Session, PyErr> {
    println!("connecting to session");
    let mut builder = RawSession::builder();
    builder = builder.clone().authtoken_from_env();

    if let Some(dict) = kwargs {
        if let Some(metadata) = dict.get("metadata") {
            builder = builder.clone().metadata(metadata);
        }
    }

    builder
        .connect()
        .await
        .map(Session::new)
        .map_err(|e| PyValueError::new_err(e.to_string()))
}

async fn internal_start_tunnel(
    session: &Session,
    kwargs: Option<HashMap<String, String>>,
) -> Result<Tunnel, PyErr> {
    println!("starting a tunnel");
    // TODO: toggle tunnel type with an enum or different functions
    let mut config = session.raw_session.tcp_endpoint();

    if let Some(dict) = kwargs {
        if let Some(metadata) = dict.get("metadata") {
            config = config.clone().metadata(metadata);
        }
        if let Some(remote_addr) = dict.get("remote_addr") {
            config = config.clone().remote_addr(remote_addr);
        }
    }

    config
        .listen()
        .await
        .map(Tunnel::new)
        .map_err(|e| PyValueError::new_err(e.to_string()))
}

async fn internal_accept(tunnel: &mut Tunnel) -> Result<Conn, PyErr> {
    tunnel
        .raw_tunnel
        .lock()
        .await
        .try_next()
        .await
        .map(|c| Conn::new(c.unwrap()))
        .map_err(|e| PyValueError::new_err(e.to_string()))
}

async fn internal_forward_tcp(tunnel: &mut Tunnel, addr: String) -> Result<(), PyErr> {
    tunnel
        .raw_tunnel
        .lock()
        .await
        .forward_tcp(addr)
        .await
        .map_err(|e| PyValueError::new_err(e.to_string()))
}

async fn internal_forward_unix(tunnel: &mut Tunnel, addr: String) -> Result<(), PyErr> {
    tunnel
        .raw_tunnel
        .lock()
        .await
        .forward_unix(addr)
        .await
        .map_err(|e| PyValueError::new_err(e.to_string()))
}

#[pyfunction(py_kwargs = "**")]
#[allow(clippy::needless_lifetimes)] // clippy has its limits, these are required
fn connect<'a>(py: Python<'a>, py_kwargs: Option<&PyDict>) -> PyResult<&'a PyAny> {
    let map = py_kwargs.map(|k| k.extract().unwrap());
    pyo3_asyncio::tokio::future_into_py(py, async move { internal_connect(map).await })
}

#[pyclass]
#[derive(Clone)]
struct Tunnel {
    url: String,
    raw_tunnel: Arc<Mutex<TcpTunnel>>,
}

impl Tunnel {
    fn new(raw_tunnel: TcpTunnel) -> Self {
        Tunnel {
            url: raw_tunnel.inner.url.clone(),
            raw_tunnel: Arc::new(Mutex::new(raw_tunnel)),
        }
    }
}

#[pymethods]
impl Tunnel {
    fn __str__(&self) -> String {
        self.url.clone()
    }

    pub fn read_line(&self) -> String {
        "".to_string()
    }

    pub fn bind(&self, _unused: String) {
        println!("bind");
    }

    pub fn accept(&mut self) -> Result<Conn, PyErr> {
        println!("accept");
        block_on(async { internal_accept(self).await })
    }

    pub fn forward_tcp<'a>(&mut self, py: Python<'a>, addr: String) -> PyResult<&'a PyAny> {
        println!("forward_tcp");
        let mut my_tunnel = self.clone();
        pyo3_asyncio::tokio::future_into_py(py, async move {
            internal_forward_tcp(&mut my_tunnel, addr.clone()).await
        })
    }

    pub fn forward_unix<'a>(&mut self, py: Python<'a>, addr: String) -> PyResult<&'a PyAny> {
        println!("forward_unix");
        let mut my_tunnel = self.clone();
        pyo3_asyncio::tokio::future_into_py(py, async move {
            internal_forward_unix(&mut my_tunnel, addr.clone()).await
        })
    }

    pub fn fileno(&self) -> usize {
        println!("fileno");
        9
    }
}

#[pyclass(subclass, name = "RawIOBase")]
#[derive(Clone)]
pub struct Conn {
    closed: bool,
    remote_addr: String,
    reader: Arc<Mutex<ReadHalf<RawConn>>>,
    writer: Arc<Mutex<WriteHalf<RawConn>>>,
}

impl Conn {
    fn new(raw_conn: RawConn) -> Self {
        let remote_addr = raw_conn.remote_addr.to_string();
        let (rx, tx) = io::split(raw_conn);
        Conn {
            closed: false,
            remote_addr,
            reader: Arc::new(Mutex::new(rx)),
            writer: Arc::new(Mutex::new(tx)),
        }
    }
}

#[pymethods]
// satisfies https://docs.python.org/3/library/io.html#io.RawIOBase
impl Conn {
    fn __str__(&self) -> String {
        self.remote_addr.clone()
    }

    #[getter]
    pub fn get_closed(&self) -> bool {
        self.closed
    }

    pub fn readable(&self) -> bool {
        true
    }

    pub fn seekable(&self) -> bool {
        false
    }

    pub fn writable(&self) -> bool {
        true
    }

    pub fn recv_fixed<'a>(&self, py: Python<'a>) -> PyResult<&'a PyAny> {
        let reader = self.reader.clone();
        pyo3_asyncio::tokio::future_into_py(py, async move {
            // sigh, pyo3 turns this into a list too
            let mut buffer = [0u8; 32];
            let res = reader.lock().await.read(&mut buffer).await;
            res.map(move |_size| buffer)
                .map_err(|e| PyValueError::new_err(e.to_string()))
        })
    }

    pub fn recv<'a>(&self, py: Python<'a>, max_size: usize) -> PyResult<&'a PyAny> {
        let reader = self.reader.clone();
        pyo3_asyncio::tokio::future_into_py(py, async move {
            let mut buffer = BytesMut::with_capacity(max_size);
            let res = reader.lock().await.read_buf(&mut buffer).await;
            // if res.is_ok() {
            //     // error: Returning this value requires that `’1` must outlive `’2` :(
            //     // https://users.rust-lang.org/t/returning-this-value-requires-that-1-must-outlive-2/51417/8
            //     // Also can't use the 'py' above because of the async boundary:
            //     // error: "*mut pyo3::Python<'static>` cannot be sent between threads safely"
            //     // Doc examples never returns anything interesting: https://pyo3.rs/main/ecosystem/async-await.html
            //     // Long discussion without help for this case: https://github.com/PyO3/pyo3/issues/1385
            //     // List of ways, but all require py:
            //     // https://stackoverflow.com/questions/73409739/what-are-the-differences-between-these-4-methods-of-returning-bytes-from-rust
            //     return Ok(Python::with_gil(|py| return PyByteArray::new(py, &buffer[..])));
            //     return Ok(PyByteArray::new(py, &buffer[..]));
            // }

            res.map(move |_size| buffer.to_vec()) // this vec becomes a list-of-int, want bytearray but problems above
                .map_err(|e| PyValueError::new_err(e.to_string()))
        })
    }

    // buffer.as_bytes_mut() is unsafe
    pub unsafe fn readinto(&self, buffer: &PyByteArray) -> usize {
        // println!("input: {} type: {}", input, input.get_type());
        // let mut buffer = String::new();
        let reader = self.reader.clone();
        // need some blocking i/o
        block_on(async {
            let res = reader.lock().await.read(buffer.as_bytes_mut()).await;
            if let Ok(size) = res {
                return size;
            }
            0
        })
    }

    // buffer.as_bytes() is unsafe
    pub unsafe fn write(&self, buffer: &PyByteArray) -> usize {
        let writer = self.writer.clone();
        // need some blocking i/o
        block_on(async {
            let res = writer.lock().await.write(buffer.as_bytes()).await;
            if let Ok(size) = res {
                return size;
            }
            0
        })
    }
}

/// A Python module implemented in Rust. The name of this function must match
/// the `lib.name` setting in the `Cargo.toml`, else Python will not be able to
/// import the module.
#[pymodule]
fn ngrok(_py: Python<'_>, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(connect, m)?)?;
    m.add_class::<Tunnel>()?;
    Ok(())
}
