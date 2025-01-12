use std::path::PathBuf;
use std::pin::Pin;
use std::future::Future;
use log::{info, error};
use pingora::server::Server;
use pingora::services::Service;
use pingora::protocols::http::ServerSession;
use crate::dns::LocalDnsResolver;
use crate::fastcgi::FastCGIHandler;
use tokio::net::UnixStream;
use std::collections::HashMap;

pub struct FastCGIServer {
    dns_resolver: LocalDnsResolver,
    fastcgi_socket: PathBuf,
}

impl FastCGIServer {
    pub fn new(dns_resolver: LocalDnsResolver, fastcgi_socket: PathBuf) -> Self {
        FastCGIServer {
            dns_resolver,
            fastcgi_socket,
        }
    }

    pub async fn run(&self) -> std::io::Result<()> {
        let mut server = Server::new(None)?;
        let service = ProxyService::new(self.dns_resolver.clone(), self.fastcgi_socket.clone());
        server.add_service(service);
        info!("FastCGI server listening on 127.0.0.1:8080");
        server.run_forever().await?;
        Ok(())
    }
}

pub struct ProxyService {
    dns_resolver: LocalDnsResolver,
    fastcgi_socket: PathBuf,
}

impl ProxyService {
    pub fn new(dns_resolver: LocalDnsResolver, fastcgi_socket: PathBuf) -> Self {
        ProxyService {
            dns_resolver,
            fastcgi_socket,
        }
    }
}

impl Service for ProxyService {
    fn name(&self) -> &str {
        "FastCGI Proxy"
    }

    fn start_service(&self, _fds: Vec<std::os::unix::io::RawFd>, _shutdown: Box<dyn Fn() + Send + Sync>) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        Box::pin(async {})
    }

    fn handle_tcp_session(&self, mut session: ServerSession) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        let dns_resolver = self.dns_resolver.clone();
        let fastcgi_socket = self.fastcgi_socket.clone();
        let handler = FastCGIHandler::new();

        Box::pin(async move {
            // Get host from request
            let host = match session.req_header().uri.host() {
                Some(h) => h.to_string(),
                None => {
                    error!("No host in request");
                    session.respond_error(400, Some("Bad Request")).await;
                    return;
                }
            };

            // Get document root
            let document_root = match dns_resolver.get_document_root(&host).await {
                Some(root) => root,
                None => {
                    error!("Document root not found for host: {}", host);
                    session.respond_error(404, Some("Not Found")).await;
                    return;
                }
            };

            // Connect to FastCGI socket
            let stream = match UnixStream::connect(&fastcgi_socket).await {
                Ok(s) => s,
                Err(e) => {
                    error!("Failed to connect to FastCGI socket: {}", e);
                    session.respond_error(502, Some("Bad Gateway")).await;
                    return;
                }
            };

            // Create FastCGI parameters
            let mut params = HashMap::new();
            params.insert("SCRIPT_FILENAME".to_string(), document_root.join("index.php").to_string_lossy().to_string());
            params.insert("SCRIPT_NAME".to_string(), "/index.php".to_string());
            params.insert("REQUEST_METHOD".to_string(), session.req_header().method.to_string());
            params.insert("REQUEST_URI".to_string(), session.req_header().uri.path().to_string());
            params.insert("QUERY_STRING".to_string(), session.req_header().uri.query().unwrap_or("").to_string());
            params.insert("SERVER_PROTOCOL".to_string(), "HTTP/1.1".to_string());
            params.insert("GATEWAY_INTERFACE".to_string(), "CGI/1.1".to_string());
            params.insert("SERVER_SOFTWARE".to_string(), "rust-fastcgi-worker".to_string());

            // Add HTTP headers as FastCGI parameters
            for (name, value) in session.req_header().headers.iter() {
                let header_name = format!("HTTP_{}", name.as_str().to_uppercase().replace('-', "_"));
                params.insert(header_name, value.to_str().unwrap_or("").to_string());
            }

            // Handle FastCGI request
            if let Err(e) = handler.handle_request(params, stream).await {
                error!("Failed to handle FastCGI request: {}", e);
                session.respond_error(502, Some("Bad Gateway")).await;
            }
        })
    }
}

impl Clone for ProxyService {
    fn clone(&self) -> Self {
        ProxyService {
            dns_resolver: self.dns_resolver.clone(),
            fastcgi_socket: self.fastcgi_socket.clone(),
        }
    }
}
