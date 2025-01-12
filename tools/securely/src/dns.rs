use std::collections::HashMap;
use std::path::PathBuf;
use std::net::IpAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info};
use hickory_resolver::AsyncResolver;
use hickory_resolver::system_conf::read_system_conf;
use hickory_resolver::config::{ResolverConfig, ResolverOpts};
use hickory_resolver::TokioAsyncResolver;

#[derive(Clone)]
pub struct SiteConfig {
    pub domain: String,
    pub document_root: PathBuf,
    pub ip: IpAddr,
}

#[derive(Clone)]
pub struct LocalDnsResolver {
    resolver: Arc<TokioAsyncResolver>,
    site_configs: Arc<RwLock<HashMap<String, SiteConfig>>>,
}

impl LocalDnsResolver {
    pub async fn new() -> std::io::Result<Self> {
        let (config, opts) = read_system_conf()?;
        let resolver = TokioAsyncResolver::tokio(config, opts);

        Ok(Self {
            resolver: Arc::new(resolver),
            site_configs: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    pub async fn register_site(&self, site: SiteConfig) {
        let mut configs = self.site_configs.write().await;
        let domain = site.domain.clone();
        configs.insert(domain.clone(), site);
        debug!("Registered site {}", domain);
    }

    pub async fn get_document_root(&self, domain: &str) -> Option<PathBuf> {
        let configs = self.site_configs.read().await;
        configs.get(domain).map(|site| site.document_root.clone())
    }

    pub async fn resolve(&self, domain: &str) -> Option<IpAddr> {
        // First check our local site configs
        let configs = self.site_configs.read().await;
        if let Some(site) = configs.get(domain) {
            return Some(site.ip);
        }

        // Fall back to system DNS
        match self.resolver.lookup_ip(domain).await {
            Ok(response) => response.iter().next(),
            Err(e) => {
                error!("DNS lookup failed for {}: {}", domain, e);
                None
            }
        }
    }

    pub async fn clear(&self) {
        let mut configs = self.site_configs.write().await;
        configs.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    #[tokio::test]
    async fn test_site_registration() {
        let resolver = LocalDnsResolver::new().await.unwrap();
        let test_domain = "myapp.test";
        let test_root = PathBuf::from("/var/www/myapp");
        let test_ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));

        let site = SiteConfig {
            domain: test_domain.to_string(),
            document_root: test_root.clone(),
            ip: test_ip,
        };

        resolver.register_site(site).await;
        let resolved_root = resolver.get_document_root(test_domain).await;
        assert_eq!(resolved_root, Some(test_root));
    }

    #[tokio::test]
    async fn test_dns_resolution() {
        let resolver = LocalDnsResolver::new().await.unwrap();
        // Try to resolve a known domain
        let result = resolver.resolve("google.com").await;
        assert!(result.is_some());
    }
}
