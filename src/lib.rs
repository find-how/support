pub mod config;
pub mod error;
pub mod utils;
pub mod backends;

pub use config::Repository as Config;
pub use error::Error;

use bytes::Bytes;
use std::path::Path;

pub trait Store: Send + Sync {
    fn get(&self, key: &str) -> Result<Option<Bytes>, Error>;
    fn set(&self, key: &str, value: Bytes) -> Result<(), Error>;
    fn delete(&self, key: &str) -> Result<(), Error>;
    fn exists(&self, key: &str) -> Result<bool, Error>;
    fn clear(&self) -> Result<(), Error>;
}
