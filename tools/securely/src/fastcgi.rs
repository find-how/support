use tokio_fastcgi::{Request, Response};
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use std::collections::HashMap;

pub struct FastCGIHandler;

impl FastCGIHandler {
    pub fn new() -> Self {
        FastCGIHandler
    }

    pub async fn handle_request<W>(&self, params: HashMap<String, String>, mut writer: W) -> std::io::Result<()>
    where
        W: AsyncRead + AsyncWrite + Unpin,
    {
        // Create FastCGI request
        let request = Request::new(1, 1);

        // Add parameters
        for (key, value) in params {
            request.set_param(&key, &value);
        }

        // Write request
        request.write(&mut writer).await?;

        // Read response
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        loop {
            match Response::read(&mut writer).await? {
                Response::Stdout(data) => {
                    stdout.extend_from_slice(&data);
                }
                Response::Stderr(data) => {
                    stderr.extend_from_slice(&data);
                    eprintln!("FastCGI stderr: {}", String::from_utf8_lossy(&data));
                }
                Response::End(_) => break,
                _ => {}
            }
        }

        // Write response
        writer.write_all(&stdout).await?;

        Ok(())
    }
}
