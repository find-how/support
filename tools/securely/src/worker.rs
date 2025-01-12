use std::path::PathBuf;
use tokio::net::UnixStream;
use std::collections::HashMap;
use log::{info, error};
use crate::fastcgi::FastCGIHandler;
use crate::dns::LocalDnsResolver;

pub struct Worker {
    default_root: PathBuf,
    dns_resolver: LocalDnsResolver,
}

impl Worker {
    pub fn new(default_root: PathBuf, dns_resolver: LocalDnsResolver) -> Self {
        Worker {
            default_root,
            dns_resolver,
        }
    }

    pub async fn handle_connection(&mut self, mut stream: UnixStream) -> std::io::Result<()> {
        // Create FastCGI parameters
        let mut params = HashMap::new();

        // Get host from parameters
        let host = params.get("HTTP_HOST").map(|s| s.to_string());

        // Get document root
        let script_filename = if let Some(host) = host {
            if let Some(root) = self.dns_resolver.get_document_root(&host).await {
                root
            } else {
                self.default_root.clone()
            }
        } else {
            self.default_root.clone()
        };

        // Add script filename to parameters
        params.insert("SCRIPT_FILENAME".to_string(), script_filename.to_string_lossy().to_string());

        // Handle FastCGI request
        let handler = FastCGIHandler::new();
        if let Err(e) = handler.handle_request(&mut stream, params).await {
            error!("FastCGI request failed: {}", e);
            return Err(std::io::Error::new(std::io::ErrorKind::Other, e));
        }

        Ok(())
    }
}
