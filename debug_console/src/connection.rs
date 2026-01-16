//! Debug server connection handling

use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

/// Connection to a debug server
pub struct DebugConnection {
    stream: UnixStream,
}

impl DebugConnection {
    /// Connect to debug server at socket path
    pub async fn connect(socket_path: PathBuf) -> Result<Self, std::io::Error> {
        let stream = UnixStream::connect(&socket_path).await?;
        Ok(Self { stream })
    }

    /// Send a command and receive response
    pub async fn send_command(&mut self, cmd: &str) -> Result<String, std::io::Error> {
        // Split stream for reading and writing
        let (reader, mut writer) = self.stream.split();

        // Send command
        writer.write_all(cmd.as_bytes()).await?;
        writer.write_all(b"\n").await?;
        writer.flush().await?;

        // Read response
        let mut buf_reader = BufReader::new(reader);
        let mut response = String::new();
        buf_reader.read_line(&mut response).await?;

        Ok(response.trim().to_string())
    }
}
