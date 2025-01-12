use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use http_body_util::Full;
use bytes::Bytes;
use tokio::net::{TcpListener, UnixStream};
use std::convert::Infallible;
use std::path::Path;
use log::{info, error};

pub struct HttpServer {
    fastcgi_socket: String,
}

impl HttpServer {
    pub fn new(fastcgi_socket: String) -> Self {
        Self { fastcgi_socket }
    }

    pub async fn run(&self, addr: &str) -> Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind(addr).await?;
        info!("HTTP server listening on {}", addr);

        let fastcgi_socket = self.fastcgi_socket.clone();

        loop {
            let (tcp, _) = listener.accept().await?;
            let io = TokioIo::new(tcp);
            let fastcgi_socket = fastcgi_socket.clone();

            tokio::task::spawn(async move {
                let service = service_fn(move |req| {
                    let fastcgi_socket = fastcgi_socket.clone();
                    async move { handle_request(req, &fastcgi_socket).await }
                });

                if let Err(err) = http1::Builder::new()
                    .serve_connection(io, service)
                    .await
                {
                    error!("Error serving connection: {}", err);
                }
            });
        }
    }
}

async fn handle_request(
    req: Request<hyper::body::Incoming>,
    fastcgi_socket: &str,
) -> Result<Response<Full<Bytes>>, Infallible> {
    // Connect to FastCGI socket
    match UnixStream::connect(fastcgi_socket).await {
        Ok(mut stream) => {
            // TODO: Convert HTTP request to FastCGI request
            // For now, just return a placeholder response
            Ok(Response::builder()
                .status(StatusCode::OK)
                .body(Full::new(Bytes::from("FastCGI connection successful")))
                .unwrap())
        }
        Err(e) => {
            error!("Failed to connect to FastCGI socket: {}", e);
            Ok(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Full::new(Bytes::from("Internal Server Error")))
                .unwrap())
        }
    }
}
