use tokio::net::UnixDatagram;

pub struct SdNotify {
    sock: Option<UnixDatagram>,
}

impl SdNotify {
    pub fn new() -> std::io::Result<Self> {
        let Some(socket_path) = std::env::var_os("NOTIFY_SOCKET") else {
            return Ok(Self { sock: None });
        };

        let sock = tokio::net::UnixDatagram::unbound()?;
        sock.connect(socket_path)?;
        Ok(Self { sock: Some(sock) })
    }

    pub async fn ready(&self) -> std::io::Result<()> {
        let Some(sock) = &self.sock else {
            return Ok(());
        };
        sock.send(b"READY=1").await?;
        Ok(())
    }
}
