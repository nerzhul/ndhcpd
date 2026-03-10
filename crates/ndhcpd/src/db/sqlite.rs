use crate::models::{ApiToken, DynamicRange, IAPrefix, Lease, StaticIP, Subnet};
use sqlx::{sqlite::SqliteConnectOptions, Row, SqlitePool};
use std::str::FromStr;

use super::Database;

/// SQLite implementation of the Database trait
pub struct SqliteDatabase {
    pool: SqlitePool,
}

impl SqliteDatabase {
    /// Create a new SQLite database connection
    pub async fn new(database_url: &str) -> anyhow::Result<Self> {
        let options = SqliteConnectOptions::from_str(database_url)?.create_if_missing(true);

        let pool = SqlitePool::connect_with(options).await?;

        // Run migrations
        sqlx::migrate!("./migrations").run(&pool).await?;

        Ok(Self { pool })
    }

    /// Get the underlying connection pool
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}

#[async_trait::async_trait]
impl Database for SqliteDatabase {
    // Subnet operations
    async fn create_subnet(&self, subnet: &Subnet) -> anyhow::Result<i64> {
        let dns_servers = subnet.dns_servers_to_string();
        let result = sqlx::query(
            "INSERT INTO subnets (network, netmask, gateway, dns_servers, domain_name, enabled) VALUES (?, ?, ?, ?, ?, ?)"
        )
        .bind(subnet.network.to_string())
        .bind(subnet.netmask as i64)
        .bind(subnet.gateway.to_string())
        .bind(dns_servers)
        .bind(&subnet.domain_name)
        .bind(subnet.enabled as i64)
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    async fn get_subnet(&self, id: i64) -> anyhow::Result<Option<Subnet>> {
        let row = sqlx::query(
            "SELECT id, network, netmask, gateway, dns_servers, domain_name, enabled FROM subnets WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| Subnet {
            id: r.get("id"),
            network: r.get::<String, _>("network").parse().unwrap(),
            netmask: r.get::<i64, _>("netmask") as u8,
            gateway: r.get::<String, _>("gateway").parse().unwrap(),
            dns_servers: Subnet::dns_servers_from_string(&r.get::<String, _>("dns_servers")),
            domain_name: r.get("domain_name"),
            enabled: r.get::<i64, _>("enabled") != 0,
        }))
    }

    async fn list_subnets(&self) -> anyhow::Result<Vec<Subnet>> {
        let rows = sqlx::query(
            "SELECT id, network, netmask, gateway, dns_servers, domain_name, enabled FROM subnets",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| Subnet {
                id: r.get("id"),
                network: r.get::<String, _>("network").parse().unwrap(),
                netmask: r.get::<i64, _>("netmask") as u8,
                gateway: r.get::<String, _>("gateway").parse().unwrap(),
                dns_servers: Subnet::dns_servers_from_string(&r.get::<String, _>("dns_servers")),
                domain_name: r.get("domain_name"),
                enabled: r.get::<i64, _>("enabled") != 0,
            })
            .collect())
    }

    async fn list_active_subnets(&self) -> anyhow::Result<Vec<Subnet>> {
        let rows = sqlx::query(
            "SELECT id, network, netmask, gateway, dns_servers, domain_name, enabled FROM subnets WHERE enabled = 1",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| Subnet {
                id: r.get("id"),
                network: r.get::<String, _>("network").parse().unwrap(),
                netmask: r.get::<i64, _>("netmask") as u8,
                gateway: r.get::<String, _>("gateway").parse().unwrap(),
                dns_servers: Subnet::dns_servers_from_string(&r.get::<String, _>("dns_servers")),
                domain_name: r.get("domain_name"),
                enabled: true,
            })
            .collect())
    }

    async fn update_subnet(&self, id: i64, subnet: &Subnet) -> anyhow::Result<()> {
        let dns_servers = subnet.dns_servers_to_string();
        sqlx::query(
            "UPDATE subnets SET network = ?, netmask = ?, gateway = ?, dns_servers = ?, domain_name = ?, enabled = ? WHERE id = ?"
        )
        .bind(subnet.network.to_string())
        .bind(subnet.netmask as i64)
        .bind(subnet.gateway.to_string())
        .bind(dns_servers)
        .bind(&subnet.domain_name)
        .bind(subnet.enabled as i64)
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn delete_subnet(&self, id: i64) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM subnets WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // Dynamic Range operations
    async fn create_range(&self, range: &DynamicRange) -> anyhow::Result<i64> {
        let result = sqlx::query(
            "INSERT INTO dynamic_ranges (subnet_id, range_start, range_end, enabled) VALUES (?, ?, ?, ?)"
        )
        .bind(range.subnet_id)
        .bind(range.range_start.to_string())
        .bind(range.range_end.to_string())
        .bind(range.enabled as i64)
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    async fn list_ranges(&self, subnet_id: Option<i64>) -> anyhow::Result<Vec<DynamicRange>> {
        let rows = if let Some(subnet_id) = subnet_id {
            sqlx::query(
                "SELECT id, subnet_id, range_start, range_end, enabled FROM dynamic_ranges WHERE subnet_id = ?"
            )
            .bind(subnet_id)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query("SELECT id, subnet_id, range_start, range_end, enabled FROM dynamic_ranges")
                .fetch_all(&self.pool)
                .await?
        };

        Ok(rows
            .into_iter()
            .map(|r| DynamicRange {
                id: r.get("id"),
                subnet_id: r.get("subnet_id"),
                range_start: r.get::<String, _>("range_start").parse().unwrap(),
                range_end: r.get::<String, _>("range_end").parse().unwrap(),
                enabled: r.get::<i64, _>("enabled") != 0,
            })
            .collect())
    }

    async fn delete_range(&self, id: i64) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM dynamic_ranges WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // Static IP operations
    async fn create_static_ip(&self, static_ip: &StaticIP) -> anyhow::Result<i64> {
        let result = sqlx::query(
            "INSERT INTO static_ips (subnet_id, mac_address, ip_address, hostname, enabled) VALUES (?, ?, ?, ?, ?)"
        )
        .bind(static_ip.subnet_id)
        .bind(&static_ip.mac_address)
        .bind(static_ip.ip_address.to_string())
        .bind(&static_ip.hostname)
        .bind(static_ip.enabled as i64)
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    async fn list_static_ips(&self, subnet_id: Option<i64>) -> anyhow::Result<Vec<StaticIP>> {
        let rows = if let Some(subnet_id) = subnet_id {
            sqlx::query(
                "SELECT id, subnet_id, mac_address, ip_address, hostname, enabled FROM static_ips WHERE subnet_id = ?"
            )
            .bind(subnet_id)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query(
                "SELECT id, subnet_id, mac_address, ip_address, hostname, enabled FROM static_ips",
            )
            .fetch_all(&self.pool)
            .await?
        };

        Ok(rows
            .into_iter()
            .map(|r| StaticIP {
                id: r.get("id"),
                subnet_id: r.get("subnet_id"),
                mac_address: r.get("mac_address"),
                ip_address: r.get::<String, _>("ip_address").parse().unwrap(),
                hostname: r.get("hostname"),
                enabled: r.get::<i64, _>("enabled") != 0,
            })
            .collect())
    }

    async fn get_static_ip_by_mac(&self, mac: &str) -> anyhow::Result<Option<StaticIP>> {
        let row = sqlx::query(
            "SELECT id, subnet_id, mac_address, ip_address, hostname, enabled FROM static_ips WHERE mac_address = ? AND enabled = 1"
        )
        .bind(mac)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| StaticIP {
            id: r.get("id"),
            subnet_id: r.get("subnet_id"),
            mac_address: r.get("mac_address"),
            ip_address: r.get::<String, _>("ip_address").parse().unwrap(),
            hostname: r.get("hostname"),
            enabled: r.get::<i64, _>("enabled") != 0,
        }))
    }

    async fn delete_static_ip(&self, id: i64) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM static_ips WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // Lease operations
    async fn create_lease(&self, lease: &Lease) -> anyhow::Result<i64> {
        let result = sqlx::query(
            "INSERT INTO leases (subnet_id, mac_address, ip_address, lease_start, lease_end, hostname, active) VALUES (?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(lease.subnet_id)
        .bind(&lease.mac_address)
        .bind(lease.ip_address.to_string())
        .bind(lease.lease_start)
        .bind(lease.lease_end)
        .bind(&lease.hostname)
        .bind(lease.active as i64)
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    async fn get_active_lease(&self, mac: &str) -> anyhow::Result<Option<Lease>> {
        let now = chrono::Utc::now().timestamp();
        let row = sqlx::query(
            "SELECT id, subnet_id, mac_address, ip_address, lease_start, lease_end, hostname, active FROM leases WHERE mac_address = ? AND active = 1 AND lease_end > ? ORDER BY lease_end DESC LIMIT 1"
        )
        .bind(mac)
        .bind(now)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| Lease {
            id: r.get("id"),
            subnet_id: r.get("subnet_id"),
            mac_address: r.get("mac_address"),
            ip_address: r.get::<String, _>("ip_address").parse().unwrap(),
            lease_start: r.get("lease_start"),
            lease_end: r.get("lease_end"),
            hostname: r.get("hostname"),
            active: r.get::<i64, _>("active") != 0,
        }))
    }

    async fn list_active_leases(&self) -> anyhow::Result<Vec<Lease>> {
        let now = chrono::Utc::now().timestamp();
        let rows = sqlx::query(
            "SELECT id, subnet_id, mac_address, ip_address, lease_start, lease_end, hostname, active FROM leases WHERE active = 1 AND lease_end > ?"
        )
        .bind(now)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| Lease {
                id: r.get("id"),
                subnet_id: r.get("subnet_id"),
                mac_address: r.get("mac_address"),
                ip_address: r.get::<String, _>("ip_address").parse().unwrap(),
                lease_start: r.get("lease_start"),
                lease_end: r.get("lease_end"),
                hostname: r.get("hostname"),
                active: r.get::<i64, _>("active") != 0,
            })
            .collect())
    }

    async fn expire_lease(&self, id: i64) -> anyhow::Result<()> {
        sqlx::query("UPDATE leases SET active = 0 WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // IPv6 Prefix (IA Prefix) operations
    async fn create_ia_prefix(&self, prefix: &IAPrefix) -> anyhow::Result<i64> {
        let dns_servers = prefix.dns_servers_to_string();
        let result = sqlx::query(
            "INSERT INTO ia_prefixes (interface, prefix, prefix_len, preferred_lifetime, valid_lifetime, dns_servers, dns_lifetime, enabled) VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&prefix.interface)
        .bind(prefix.prefix.to_string())
        .bind(prefix.prefix_len as i64)
        .bind(prefix.preferred_lifetime as i64)
        .bind(prefix.valid_lifetime as i64)
        .bind(dns_servers)
        .bind(prefix.dns_lifetime as i64)
        .bind(prefix.enabled as i64)
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    async fn get_ia_prefix(&self, id: i64) -> anyhow::Result<Option<IAPrefix>> {
        let row = sqlx::query(
            "SELECT id, interface, prefix, prefix_len, preferred_lifetime, valid_lifetime, dns_servers, dns_lifetime, enabled FROM ia_prefixes WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| IAPrefix {
            id: r.get("id"),
            interface: r.get("interface"),
            prefix: r.get::<String, _>("prefix").parse().unwrap(),
            prefix_len: r.get::<i64, _>("prefix_len") as u8,
            preferred_lifetime: r.get::<i64, _>("preferred_lifetime") as u32,
            valid_lifetime: r.get::<i64, _>("valid_lifetime") as u32,
            dns_servers: IAPrefix::dns_servers_from_string(&r.get::<String, _>("dns_servers")),
            dns_lifetime: r.get::<i64, _>("dns_lifetime") as u32,
            enabled: r.get::<i64, _>("enabled") != 0,
        }))
    }

    async fn list_ia_prefixes(&self, interface: Option<&str>) -> anyhow::Result<Vec<IAPrefix>> {
        let rows = if let Some(interface) = interface {
            sqlx::query(
                "SELECT id, interface, prefix, prefix_len, preferred_lifetime, valid_lifetime, dns_servers, dns_lifetime, enabled FROM ia_prefixes WHERE interface = ?"
            )
            .bind(interface)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query(
                "SELECT id, interface, prefix, prefix_len, preferred_lifetime, valid_lifetime, dns_servers, dns_lifetime, enabled FROM ia_prefixes"
            )
            .fetch_all(&self.pool)
            .await?
        };

        Ok(rows
            .into_iter()
            .map(|r| IAPrefix {
                id: r.get("id"),
                interface: r.get("interface"),
                prefix: r.get::<String, _>("prefix").parse().unwrap(),
                prefix_len: r.get::<i64, _>("prefix_len") as u8,
                preferred_lifetime: r.get::<i64, _>("preferred_lifetime") as u32,
                valid_lifetime: r.get::<i64, _>("valid_lifetime") as u32,
                dns_servers: IAPrefix::dns_servers_from_string(&r.get::<String, _>("dns_servers")),
                dns_lifetime: r.get::<i64, _>("dns_lifetime") as u32,
                enabled: r.get::<i64, _>("enabled") != 0,
            })
            .collect())
    }

    async fn list_enabled_ia_prefixes(&self) -> anyhow::Result<Vec<IAPrefix>> {
        let rows = sqlx::query(
            "SELECT id, interface, prefix, prefix_len, preferred_lifetime, valid_lifetime, dns_servers, dns_lifetime, enabled FROM ia_prefixes WHERE enabled = 1"
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| IAPrefix {
                id: r.get("id"),
                interface: r.get("interface"),
                prefix: r.get::<String, _>("prefix").parse().unwrap(),
                prefix_len: r.get::<i64, _>("prefix_len") as u8,
                preferred_lifetime: r.get::<i64, _>("preferred_lifetime") as u32,
                valid_lifetime: r.get::<i64, _>("valid_lifetime") as u32,
                dns_servers: IAPrefix::dns_servers_from_string(&r.get::<String, _>("dns_servers")),
                dns_lifetime: r.get::<i64, _>("dns_lifetime") as u32,
                enabled: r.get::<i64, _>("enabled") != 0,
            })
            .collect())
    }

    async fn update_ia_prefix(&self, id: i64, prefix: &IAPrefix) -> anyhow::Result<()> {
        let dns_servers = prefix.dns_servers_to_string();
        sqlx::query(
            "UPDATE ia_prefixes SET interface = ?, prefix = ?, prefix_len = ?, preferred_lifetime = ?, valid_lifetime = ?, dns_servers = ?, dns_lifetime = ?, enabled = ? WHERE id = ?"
        )
        .bind(&prefix.interface)
        .bind(prefix.prefix.to_string())
        .bind(prefix.prefix_len as i64)
        .bind(prefix.preferred_lifetime as i64)
        .bind(prefix.valid_lifetime as i64)
        .bind(dns_servers)
        .bind(prefix.dns_lifetime as i64)
        .bind(prefix.enabled as i64)
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn delete_ia_prefix(&self, id: i64) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM ia_prefixes WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // Token operations
    async fn list_tokens(&self) -> anyhow::Result<Vec<(String, i64)>> {
        let rows = sqlx::query_as::<_, (String, i64)>(
            "SELECT token_hash, enabled FROM api_tokens WHERE enabled = 1",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    async fn list_api_tokens(&self) -> anyhow::Result<Vec<ApiToken>> {
        let rows = sqlx::query_as::<_, (i64, String, i64, Option<i64>, bool)>(
            "SELECT id, name, created_at, last_used_at, enabled FROM api_tokens ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|(id, name, created_at, last_used_at, enabled)| ApiToken {
                id: Some(id),
                name,
                token_hash: None,
                salt: None,
                created_at: Some(created_at),
                last_used_at,
                enabled,
                token: None,
            })
            .collect())
    }

    async fn create_token(&self, name: &str, token_hash: &str, salt: &str) -> anyhow::Result<i64> {
        let result = sqlx::query(
            "INSERT INTO api_tokens (name, token_hash, salt, created_at, enabled) VALUES (?, ?, ?, strftime('%s', 'now'), 1)"
        )
        .bind(name)
        .bind(token_hash)
        .bind(salt)
        .execute(&self.pool)
        .await?;
        Ok(result.last_insert_rowid())
    }

    async fn delete_token(&self, id: i64) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM api_tokens WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn toggle_token(&self, id: i64, enabled: bool) -> anyhow::Result<()> {
        sqlx::query("UPDATE api_tokens SET enabled = ? WHERE id = ?")
            .bind(enabled as i64)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn update_token_last_used(&self, token_hash: &str) -> anyhow::Result<()> {
        sqlx::query(
            "UPDATE api_tokens SET last_used_at = strftime('%s', 'now') WHERE token_hash = ?",
        )
        .bind(token_hash)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::tests::suite;

    async fn new_test_db() -> SqliteDatabase {
        SqliteDatabase::new("sqlite::memory:").await.unwrap()
    }

    #[tokio::test]
    async fn test_sqlite_database() {
        let db = new_test_db().await;
        suite::run_all(&db).await;
    }

    #[tokio::test]
    async fn test_subnet_crud() {
        let db = new_test_db().await;
        suite::test_create_and_get_subnet(&db).await;
        suite::test_list_subnets(&db).await;
        suite::test_update_subnet(&db).await;
        suite::test_delete_subnet(&db).await;
        suite::test_get_subnet_not_found(&db).await;
    }

    #[tokio::test]
    async fn test_range_crud() {
        let db = new_test_db().await;
        suite::test_create_and_list_range(&db).await;
        suite::test_list_ranges_all(&db).await;
        suite::test_delete_range(&db).await;
    }

    #[tokio::test]
    async fn test_static_ip_crud() {
        let db = new_test_db().await;
        suite::test_create_and_list_static_ip(&db).await;
        suite::test_get_static_ip_by_mac(&db).await;
        suite::test_get_static_ip_by_mac_not_found(&db).await;
        suite::test_delete_static_ip(&db).await;
    }

    #[tokio::test]
    async fn test_lease_crud() {
        let db = new_test_db().await;
        suite::test_create_and_get_active_lease(&db).await;
        suite::test_list_active_leases(&db).await;
        suite::test_expire_lease(&db).await;
        suite::test_expired_lease_not_returned(&db).await;
    }

    #[tokio::test]
    async fn test_ia_prefix_crud() {
        let db = new_test_db().await;
        suite::test_create_and_get_ia_prefix(&db).await;
        suite::test_list_ia_prefixes_by_interface(&db).await;
        suite::test_list_enabled_ia_prefixes(&db).await;
        suite::test_update_ia_prefix(&db).await;
        suite::test_delete_ia_prefix(&db).await;
    }

    #[tokio::test]
    async fn test_token_crud() {
        let db = new_test_db().await;
        suite::test_create_and_list_tokens(&db).await;
        suite::test_list_api_tokens(&db).await;
        suite::test_delete_token(&db).await;
        suite::test_toggle_token(&db).await;
        suite::test_update_token_last_used(&db).await;
    }
}
