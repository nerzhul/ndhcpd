use crate::models::{ApiToken, DynamicRange, IAPrefix, Lease, StaticIP, Subnet};
use std::sync::Arc;
use tokio::sync::RwLock;

use super::Database;

/// In-memory implementation of the Database trait (for testing)
pub struct InMemoryDatabase {
    subnets: Arc<RwLock<Vec<Subnet>>>,
    ranges: Arc<RwLock<Vec<DynamicRange>>>,
    static_ips: Arc<RwLock<Vec<StaticIP>>>,
    leases: Arc<RwLock<Vec<Lease>>>,
    ia_prefixes: Arc<RwLock<Vec<IAPrefix>>>,
    tokens: Arc<RwLock<Vec<(i64, String, String, i64)>>>, // id, name, token_hash, enabled
    next_subnet_id: Arc<RwLock<i64>>,
    next_range_id: Arc<RwLock<i64>>,
    next_static_ip_id: Arc<RwLock<i64>>,
    next_lease_id: Arc<RwLock<i64>>,
    next_ia_prefix_id: Arc<RwLock<i64>>,
    next_token_id: Arc<RwLock<i64>>,
}

impl InMemoryDatabase {
    /// Create a new in-memory database
    pub fn new() -> Self {
        Self {
            subnets: Arc::new(RwLock::new(Vec::new())),
            ranges: Arc::new(RwLock::new(Vec::new())),
            static_ips: Arc::new(RwLock::new(Vec::new())),
            leases: Arc::new(RwLock::new(Vec::new())),
            ia_prefixes: Arc::new(RwLock::new(Vec::new())),
            tokens: Arc::new(RwLock::new(Vec::new())),
            next_subnet_id: Arc::new(RwLock::new(1)),
            next_range_id: Arc::new(RwLock::new(1)),
            next_static_ip_id: Arc::new(RwLock::new(1)),
            next_lease_id: Arc::new(RwLock::new(1)),
            next_ia_prefix_id: Arc::new(RwLock::new(1)),
            next_token_id: Arc::new(RwLock::new(1)),
        }
    }
}

impl Default for InMemoryDatabase {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Database for InMemoryDatabase {
    // Subnet operations
    async fn create_subnet(&self, subnet: &Subnet) -> anyhow::Result<i64> {
        let mut id = self.next_subnet_id.write().await;
        let new_id = *id;
        *id += 1;

        let mut subnets = self.subnets.write().await;
        let mut new_subnet = subnet.clone();
        new_subnet.id = Some(new_id);
        subnets.push(new_subnet);

        Ok(new_id)
    }

    async fn get_subnet(&self, id: i64) -> anyhow::Result<Option<Subnet>> {
        let subnets = self.subnets.read().await;
        Ok(subnets.iter().find(|s| s.id == Some(id)).cloned())
    }

    async fn list_subnets(&self) -> anyhow::Result<Vec<Subnet>> {
        let subnets = self.subnets.read().await;
        Ok(subnets.clone())
    }

    async fn list_active_subnets(&self) -> anyhow::Result<Vec<Subnet>> {
        let subnets = self.subnets.read().await;
        Ok(subnets.iter().filter(|s| s.enabled).cloned().collect())
    }

    async fn update_subnet(&self, id: i64, subnet: &Subnet) -> anyhow::Result<()> {
        let mut subnets = self.subnets.write().await;
        if let Some(existing) = subnets.iter_mut().find(|s| s.id == Some(id)) {
            *existing = subnet.clone();
            existing.id = Some(id);
        }
        Ok(())
    }

    async fn delete_subnet(&self, id: i64) -> anyhow::Result<()> {
        let mut subnets = self.subnets.write().await;
        subnets.retain(|s| s.id != Some(id));
        Ok(())
    }

    // Dynamic Range operations
    async fn create_range(&self, range: &DynamicRange) -> anyhow::Result<i64> {
        let mut id = self.next_range_id.write().await;
        let new_id = *id;
        *id += 1;

        let mut ranges = self.ranges.write().await;
        let mut new_range = range.clone();
        new_range.id = Some(new_id);
        ranges.push(new_range);

        Ok(new_id)
    }

    async fn list_ranges(&self, subnet_id: Option<i64>) -> anyhow::Result<Vec<DynamicRange>> {
        let ranges = self.ranges.read().await;
        match subnet_id {
            Some(sid) => Ok(ranges
                .iter()
                .filter(|r| r.subnet_id == sid)
                .cloned()
                .collect()),
            None => Ok(ranges.clone()),
        }
    }

    async fn delete_range(&self, id: i64) -> anyhow::Result<()> {
        let mut ranges = self.ranges.write().await;
        ranges.retain(|r| r.id != Some(id));
        Ok(())
    }

    // Static IP operations
    async fn create_static_ip(&self, static_ip: &StaticIP) -> anyhow::Result<i64> {
        let mut id = self.next_static_ip_id.write().await;
        let new_id = *id;
        *id += 1;

        let mut static_ips = self.static_ips.write().await;
        let mut new_static_ip = static_ip.clone();
        new_static_ip.id = Some(new_id);
        static_ips.push(new_static_ip);

        Ok(new_id)
    }

    async fn list_static_ips(&self, subnet_id: Option<i64>) -> anyhow::Result<Vec<StaticIP>> {
        let static_ips = self.static_ips.read().await;
        match subnet_id {
            Some(sid) => Ok(static_ips
                .iter()
                .filter(|s| s.subnet_id == sid)
                .cloned()
                .collect()),
            None => Ok(static_ips.clone()),
        }
    }

    async fn get_static_ip_by_mac(&self, mac: &str) -> anyhow::Result<Option<StaticIP>> {
        let static_ips = self.static_ips.read().await;
        Ok(static_ips
            .iter()
            .find(|s| s.mac_address == mac && s.enabled)
            .cloned())
    }

    async fn delete_static_ip(&self, id: i64) -> anyhow::Result<()> {
        let mut static_ips = self.static_ips.write().await;
        static_ips.retain(|s| s.id != Some(id));
        Ok(())
    }

    // Lease operations
    async fn create_lease(&self, lease: &Lease) -> anyhow::Result<i64> {
        let mut id = self.next_lease_id.write().await;
        let new_id = *id;
        *id += 1;

        let mut leases = self.leases.write().await;
        let mut new_lease = lease.clone();
        new_lease.id = Some(new_id);
        leases.push(new_lease);

        Ok(new_id)
    }

    async fn get_active_lease(&self, mac: &str) -> anyhow::Result<Option<Lease>> {
        let now = chrono::Utc::now().timestamp();
        let leases = self.leases.read().await;
        Ok(leases
            .iter()
            .find(|l| l.mac_address == mac && l.active && l.lease_end > now)
            .cloned())
    }

    async fn list_active_leases(&self) -> anyhow::Result<Vec<Lease>> {
        let now = chrono::Utc::now().timestamp();
        let leases = self.leases.read().await;
        Ok(leases
            .iter()
            .filter(|l| l.active && l.lease_end > now)
            .cloned()
            .collect())
    }

    async fn expire_lease(&self, id: i64) -> anyhow::Result<()> {
        let mut leases = self.leases.write().await;
        if let Some(lease) = leases.iter_mut().find(|l| l.id == Some(id)) {
            lease.active = false;
        }
        Ok(())
    }

    // IPv6 Prefix (IA Prefix) operations
    async fn create_ia_prefix(&self, prefix: &IAPrefix) -> anyhow::Result<i64> {
        let mut id = self.next_ia_prefix_id.write().await;
        let new_id = *id;
        *id += 1;

        let mut ia_prefixes = self.ia_prefixes.write().await;
        let mut new_prefix = prefix.clone();
        new_prefix.id = Some(new_id);
        ia_prefixes.push(new_prefix);

        Ok(new_id)
    }

    async fn get_ia_prefix(&self, id: i64) -> anyhow::Result<Option<IAPrefix>> {
        let ia_prefixes = self.ia_prefixes.read().await;
        Ok(ia_prefixes.iter().find(|p| p.id == Some(id)).cloned())
    }

    async fn list_ia_prefixes(&self, interface: Option<&str>) -> anyhow::Result<Vec<IAPrefix>> {
        let ia_prefixes = self.ia_prefixes.read().await;
        match interface {
            Some(iface) => Ok(ia_prefixes
                .iter()
                .filter(|p| p.interface == iface)
                .cloned()
                .collect()),
            None => Ok(ia_prefixes.clone()),
        }
    }

    async fn list_enabled_ia_prefixes(&self) -> anyhow::Result<Vec<IAPrefix>> {
        let ia_prefixes = self.ia_prefixes.read().await;
        Ok(ia_prefixes.iter().filter(|p| p.enabled).cloned().collect())
    }

    async fn update_ia_prefix(&self, id: i64, prefix: &IAPrefix) -> anyhow::Result<()> {
        let mut ia_prefixes = self.ia_prefixes.write().await;
        if let Some(existing) = ia_prefixes.iter_mut().find(|p| p.id == Some(id)) {
            *existing = prefix.clone();
            existing.id = Some(id);
        }
        Ok(())
    }

    async fn delete_ia_prefix(&self, id: i64) -> anyhow::Result<()> {
        let mut ia_prefixes = self.ia_prefixes.write().await;
        ia_prefixes.retain(|p| p.id != Some(id));
        Ok(())
    }

    // Token operations
    async fn list_tokens(&self) -> anyhow::Result<Vec<(String, i64)>> {
        let tokens = self.tokens.read().await;
        Ok(tokens
            .iter()
            .filter(|(_, _, _, enabled)| *enabled == 1)
            .map(|(_, _, token_hash, enabled)| (token_hash.clone(), *enabled))
            .collect())
    }

    async fn list_api_tokens(&self) -> anyhow::Result<Vec<ApiToken>> {
        let tokens = self.tokens.read().await;
        Ok(tokens
            .iter()
            .map(|(id, name, _, enabled)| ApiToken {
                id: Some(*id),
                name: name.clone(),
                token_hash: None,
                salt: None,
                created_at: None,
                last_used_at: None,
                enabled: *enabled == 1,
                token: None,
            })
            .collect())
    }

    async fn create_token(&self, name: &str, token_hash: &str, _salt: &str) -> anyhow::Result<i64> {
        let mut id = self.next_token_id.write().await;
        let new_id = *id;
        *id += 1;

        let mut tokens = self.tokens.write().await;
        tokens.push((new_id, name.to_string(), token_hash.to_string(), 1));

        Ok(new_id)
    }

    async fn delete_token(&self, id: i64) -> anyhow::Result<()> {
        let mut tokens = self.tokens.write().await;
        tokens.retain(|(tid, _, _, _)| *tid != id);
        Ok(())
    }

    async fn toggle_token(&self, id: i64, enabled: bool) -> anyhow::Result<()> {
        let mut tokens = self.tokens.write().await;
        if let Some((_, _, _, e)) = tokens.iter_mut().find(|(tid, _, _, _)| *tid == id) {
            *e = if enabled { 1 } else { 0 };
        }
        Ok(())
    }

    async fn update_token_last_used(&self, _token_hash: &str) -> anyhow::Result<()> {
        // No-op for in-memory database
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::tests::suite;

    #[tokio::test]
    async fn test_in_memory_database() {
        let db = InMemoryDatabase::new();
        suite::run_all(&db).await;
    }

    #[tokio::test]
    async fn test_subnet_crud() {
        let db = InMemoryDatabase::new();
        suite::test_create_and_get_subnet(&db).await;
        suite::test_list_subnets(&db).await;
        suite::test_update_subnet(&db).await;
        suite::test_delete_subnet(&db).await;
        suite::test_get_subnet_not_found(&db).await;
    }

    #[tokio::test]
    async fn test_range_crud() {
        let db = InMemoryDatabase::new();
        suite::test_create_and_list_range(&db).await;
        suite::test_list_ranges_all(&db).await;
        suite::test_delete_range(&db).await;
    }

    #[tokio::test]
    async fn test_static_ip_crud() {
        let db = InMemoryDatabase::new();
        suite::test_create_and_list_static_ip(&db).await;
        suite::test_get_static_ip_by_mac(&db).await;
        suite::test_get_static_ip_by_mac_not_found(&db).await;
        suite::test_delete_static_ip(&db).await;
    }

    #[tokio::test]
    async fn test_lease_crud() {
        let db = InMemoryDatabase::new();
        suite::test_create_and_get_active_lease(&db).await;
        suite::test_list_active_leases(&db).await;
        suite::test_expire_lease(&db).await;
        suite::test_expired_lease_not_returned(&db).await;
    }

    #[tokio::test]
    async fn test_ia_prefix_crud() {
        let db = InMemoryDatabase::new();
        suite::test_create_and_get_ia_prefix(&db).await;
        suite::test_list_ia_prefixes_by_interface(&db).await;
        suite::test_list_enabled_ia_prefixes(&db).await;
        suite::test_update_ia_prefix(&db).await;
        suite::test_delete_ia_prefix(&db).await;
    }

    #[tokio::test]
    async fn test_token_crud() {
        let db = InMemoryDatabase::new();
        suite::test_create_and_list_tokens(&db).await;
        suite::test_list_api_tokens(&db).await;
        suite::test_delete_token(&db).await;
        suite::test_toggle_token(&db).await;
        suite::test_update_token_last_used(&db).await;
    }
}
