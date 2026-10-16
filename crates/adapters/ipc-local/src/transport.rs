#[cfg(unix)]
use crate::invalid;
use crate::private::RunDir;
use std::io;
#[cfg(unix)]
use std::path::Path;
use tokio::io::{AsyncRead, AsyncWrite};
pub(crate) trait Duplex: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Duplex for T {}
pub(crate) type Stream = Box<dyn Duplex>;

#[cfg(unix)]
pub(crate) struct Listener(tokio::net::UnixListener);
#[cfg(unix)]
impl Listener {
    pub fn bind(dir: &RunDir) -> io::Result<Self> {
        use std::os::unix::fs::{FileTypeExt, PermissionsExt};
        let path = dir.file("vellum.sock");
        match std::fs::symlink_metadata(&path) {
            Ok(meta) if meta.file_type().is_socket() => {
                // The process lock is already held. Refuse a live foreign listener anyway.
                if std::os::unix::net::UnixStream::connect(&path).is_ok() {
                    return Err(invalid("socket is already in use"));
                }
                std::fs::remove_file(&path)?;
            }
            Ok(_) => return Err(invalid("socket path is not a socket")),
            Err(e) if e.kind() == io::ErrorKind::NotFound => (),
            Err(e) => return Err(e),
        }
        let listener = tokio::net::UnixListener::bind(&path)?;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        Ok(Self(listener))
    }
    pub async fn accept(&mut self) -> io::Result<Stream> {
        self.0.accept().await.map(|(s, _)| Box::new(s) as Stream)
    }
}
#[cfg(unix)]
pub(crate) async fn connect(endpoint: &str) -> io::Result<Stream> {
    Ok(Box::new(
        tokio::net::UnixStream::connect(Path::new(endpoint)).await?,
    ))
}

#[cfg(windows)]
pub(crate) struct Listener {
    server: tokio::net::windows::named_pipe::NamedPipeServer,
    endpoint: String,
}
#[cfg(windows)]
impl Listener {
    pub fn bind(dir: &RunDir) -> io::Result<Self> {
        let endpoint = dir.endpoint()?;
        Ok(Self {
            server: crate::private::pipe(&endpoint, true)?,
            endpoint,
        })
    }
    pub async fn accept(&mut self) -> io::Result<Stream> {
        self.server.connect().await?;
        let next = crate::private::pipe(&self.endpoint, false)?;
        Ok(Box::new(std::mem::replace(&mut self.server, next)))
    }
}
#[cfg(windows)]
pub(crate) async fn connect(endpoint: &str) -> io::Result<Stream> {
    use tokio::net::windows::named_pipe::ClientOptions;
    loop {
        match ClientOptions::new().open(endpoint) {
            Ok(pipe) => return Ok(Box::new(pipe)),
            Err(e) if e.raw_os_error() == Some(231) => {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await
            }
            Err(e) => return Err(e),
        }
    }
}
