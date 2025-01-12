use std::path::PathBuf;
use std::net::{IpAddr, Ipv4Addr};
use log::{info, error};
use dirs::home_dir;

mod dns;
mod server;
mod fastcgi;
mod worker;

use dns::{LocalDnsResolver, SiteConfig};
use server::FastCGIServer;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    // Initialize logging
    env_logger::init();

    // Get home directory
    let home_dir = home_dir().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "Could not find home directory")
    })?;

    // Create FastCGI socket directory
    let socket_dir = home_dir.join(".rust-fastcgi");
    std::fs::create_dir_all(&socket_dir)?;

    // Set up socket path
    let socket_path = socket_dir.join("fastcgi.sock");
    if socket_path.exists() {
        std::fs::remove_file(&socket_path)?;
    }

    // Create document root
    let default_root = home_dir.join(".rust-fastcgi/sites");
    std::fs::create_dir_all(&default_root)?;

    // Initialize DNS resolver
    let dns_resolver = LocalDnsResolver::new().await?;

    // Register localhost.test domain
    let site = SiteConfig {
        domain: "localhost.test".to_string(),
        document_root: default_root.clone(),
        ip: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
    };
    dns_resolver.register_site(site).await;

    // Create and run FastCGI server
    let server = FastCGIServer::new(dns_resolver, socket_path);
    server.run().await?;

    Ok(())
}
