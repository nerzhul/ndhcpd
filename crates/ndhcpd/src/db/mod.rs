use crate::models::{ApiToken, DynamicRange, IAPrefix, Lease, StaticIP, Subnet};
use std::sync::Arc;

pub mod memory;
pub mod sqlite;
#[cfg(test)]
pub(crate) mod tests;

pub use memory::InMemoryDatabase;
pub use sqlite::SqliteDatabase;

/// Returns true if the error is a database unique constraint violation.
pub fn is_unique_violation(e: &anyhow::Error) -> bool {
    e.downcast_ref::<sqlx::Error>()
        .and_then(|e| {
            if let sqlx::Error::Database(db_err) = e {
                Some(db_err)
            } else {
                None
            }
        })
        .map(|db_err| db_err.is_unique_violation())
        .unwrap_or(false)
}

/// Database trait - defines the interface for all database implementations
#[async_trait::async_trait]
pub trait Database: Send + Sync {
    // Subnet operations
    async fn create_subnet(&self, subnet: &Subnet) -> anyhow::Result<i64>;
    async fn get_subnet(&self, id: i64) -> anyhow::Result<Option<Subnet>>;
    async fn list_subnets(&self) -> anyhow::Result<Vec<Subnet>>;
    async fn list_active_subnets(&self) -> anyhow::Result<Vec<Subnet>>;
    async fn update_subnet(&self, id: i64, subnet: &Subnet) -> anyhow::Result<()>;
    async fn delete_subnet(&self, id: i64) -> anyhow::Result<()>;

    // Dynamic Range operations
    async fn create_range(&self, range: &DynamicRange) -> anyhow::Result<i64>;
    async fn list_ranges(&self, subnet_id: Option<i64>) -> anyhow::Result<Vec<DynamicRange>>;
    async fn delete_range(&self, id: i64) -> anyhow::Result<()>;

    // Static IP operations
    async fn create_static_ip(&self, static_ip: &StaticIP) -> anyhow::Result<i64>;
    async fn list_static_ips(&self, subnet_id: Option<i64>) -> anyhow::Result<Vec<StaticIP>>;
    async fn get_static_ip_by_mac(&self, mac: &str) -> anyhow::Result<Option<StaticIP>>;
    async fn delete_static_ip(&self, id: i64) -> anyhow::Result<()>;

    // Lease operations
    async fn create_lease(&self, lease: &Lease) -> anyhow::Result<i64>;
    async fn get_active_lease(&self, mac: &str) -> anyhow::Result<Option<Lease>>;
    async fn list_active_leases(&self) -> anyhow::Result<Vec<Lease>>;
    async fn expire_lease(&self, id: i64) -> anyhow::Result<()>;

    // IPv6 Prefix (IA Prefix) operations
    async fn create_ia_prefix(&self, prefix: &IAPrefix) -> anyhow::Result<i64>;
    async fn get_ia_prefix(&self, id: i64) -> anyhow::Result<Option<IAPrefix>>;
    async fn list_ia_prefixes(&self, interface: Option<&str>) -> anyhow::Result<Vec<IAPrefix>>;
    async fn list_enabled_ia_prefixes(&self) -> anyhow::Result<Vec<IAPrefix>>;
    async fn update_ia_prefix(&self, id: i64, prefix: &IAPrefix) -> anyhow::Result<()>;
    async fn delete_ia_prefix(&self, id: i64) -> anyhow::Result<()>;

    // Token operations (for auth)
    async fn list_tokens(&self) -> anyhow::Result<Vec<(String, i64)>>;
    /// List all tokens with full metadata (for API handlers)
    async fn list_api_tokens(&self) -> anyhow::Result<Vec<ApiToken>>;
    async fn create_token(&self, name: &str, token_hash: &str, salt: &str) -> anyhow::Result<i64>;
    async fn delete_token(&self, id: i64) -> anyhow::Result<()>;
    async fn toggle_token(&self, id: i64, enabled: bool) -> anyhow::Result<()>;
    async fn update_token_last_used(&self, token_hash: &str) -> anyhow::Result<()>;
}

/// Type alias for a boxed Database trait object
pub type DynDatabase = Arc<dyn Database>;

/// Create a new database based on the URL scheme
pub async fn create_database(database_url: &str) -> anyhow::Result<DynDatabase> {
    if database_url.starts_with("memory:") || database_url == ":memory:" {
        Ok(Arc::new(InMemoryDatabase::new()) as DynDatabase)
    } else if database_url.starts_with("sqlite:") {
        Ok(Arc::new(SqliteDatabase::new(database_url).await?) as DynDatabase)
    } else {
        anyhow::bail!("Unsupported database URL scheme: {}", database_url)
    }
}
