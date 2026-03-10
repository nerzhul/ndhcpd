/// Shared test suite for all Database implementations.
/// Each test function uses unique network/MAC/prefix data so they can be
/// composed into run_all() without conflicting within the same DB instance.
#[cfg(test)]
pub(crate) mod suite {
    use crate::db::Database;
    use crate::models::{DynamicRange, IAPrefix, Lease, StaticIP, Subnet};
    use std::net::{Ipv4Addr, Ipv6Addr};

    fn subnet(third_octet: u8) -> Subnet {
        Subnet {
            id: None,
            network: Ipv4Addr::new(10, 0, third_octet, 0),
            netmask: 24,
            gateway: Ipv4Addr::new(10, 0, third_octet, 1),
            dns_servers: vec![Ipv4Addr::new(8, 8, 8, 8)],
            domain_name: Some("local".to_string()),
            enabled: true,
        }
    }

    fn range(subnet_id: i64, third_octet: u8) -> DynamicRange {
        DynamicRange {
            id: None,
            subnet_id,
            range_start: Ipv4Addr::new(10, 0, third_octet, 100),
            range_end: Ipv4Addr::new(10, 0, third_octet, 200),
            enabled: true,
        }
    }

    fn static_ip(subnet_id: i64, mac_suffix: &str, third_octet: u8) -> StaticIP {
        StaticIP {
            id: None,
            subnet_id,
            mac_address: format!("aa:bb:cc:dd:ee:{mac_suffix}"),
            ip_address: Ipv4Addr::new(10, 0, third_octet, 50),
            hostname: Some("test-host".to_string()),
            enabled: true,
        }
    }

    fn active_lease(subnet_id: i64, mac_suffix: &str, third_octet: u8) -> Lease {
        let now = chrono::Utc::now().timestamp();
        Lease {
            id: None,
            subnet_id,
            mac_address: format!("aa:bb:cc:dd:ee:{mac_suffix}"),
            ip_address: Ipv4Addr::new(10, 0, third_octet, 80),
            lease_start: now,
            lease_end: now + 3600,
            hostname: Some("test-host".to_string()),
            active: true,
        }
    }

    fn ia_prefix(iface: &str, prefix_group: u16) -> IAPrefix {
        IAPrefix {
            id: None,
            interface: iface.to_string(),
            prefix: Ipv6Addr::new(0x2001, 0xdb8, prefix_group, 0, 0, 0, 0, 0),
            prefix_len: 64,
            preferred_lifetime: 86400,
            valid_lifetime: 2592000,
            dns_servers: vec![Ipv6Addr::new(0x2001, 0xdb8, prefix_group, 0, 0, 0, 0, 1)],
            dns_lifetime: 3600,
            enabled: true,
        }
    }

    // --- Subnet tests ---

    pub async fn test_create_and_get_subnet(db: &dyn Database) {
        let id = db.create_subnet(&subnet(1)).await.unwrap();
        assert!(id > 0);

        let got = db.get_subnet(id).await.unwrap().expect("subnet not found");
        assert_eq!(got.id, Some(id));
        assert_eq!(got.network, Ipv4Addr::new(10, 0, 1, 0));
        assert_eq!(got.netmask, 24);
        assert_eq!(got.gateway, Ipv4Addr::new(10, 0, 1, 1));
        assert_eq!(got.dns_servers, vec![Ipv4Addr::new(8, 8, 8, 8)]);
        assert_eq!(got.domain_name, Some("local".to_string()));
        assert!(got.enabled);
    }

    pub async fn test_list_subnets(db: &dyn Database) {
        db.create_subnet(&subnet(2)).await.unwrap();
        db.create_subnet(&subnet(3)).await.unwrap();

        let list = db.list_subnets().await.unwrap();
        assert!(list.len() >= 2);
    }

    pub async fn test_list_active_subnets(db: &dyn Database) {
        let mut disabled = subnet(6);
        disabled.enabled = false;

        db.create_subnet(&subnet(7)).await.unwrap();
        let disabled_id = db.create_subnet(&disabled).await.unwrap();

        let active = db.list_active_subnets().await.unwrap();
        assert!(active.iter().all(|s| s.enabled));
        assert!(!active.iter().any(|s| s.id == Some(disabled_id)));
    }

    pub async fn test_update_subnet(db: &dyn Database) {
        let id = db.create_subnet(&subnet(4)).await.unwrap();

        let mut updated = subnet(4);
        updated.netmask = 16;
        updated.enabled = false;
        db.update_subnet(id, &updated).await.unwrap();

        let got = db.get_subnet(id).await.unwrap().expect("subnet not found");
        assert_eq!(got.netmask, 16);
        assert!(!got.enabled);
    }

    pub async fn test_delete_subnet(db: &dyn Database) {
        let id = db.create_subnet(&subnet(5)).await.unwrap();
        db.delete_subnet(id).await.unwrap();

        assert!(db.get_subnet(id).await.unwrap().is_none());
    }

    pub async fn test_get_subnet_not_found(db: &dyn Database) {
        assert!(db.get_subnet(99999).await.unwrap().is_none());
    }

    // --- Dynamic Range tests ---

    pub async fn test_create_and_list_range(db: &dyn Database) {
        let sid = db.create_subnet(&subnet(10)).await.unwrap();
        let id = db.create_range(&range(sid, 10)).await.unwrap();
        assert!(id > 0);

        let ranges = db.list_ranges(Some(sid)).await.unwrap();
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].subnet_id, sid);
        assert_eq!(ranges[0].range_start, Ipv4Addr::new(10, 0, 10, 100));
        assert_eq!(ranges[0].range_end, Ipv4Addr::new(10, 0, 10, 200));
    }

    pub async fn test_list_ranges_all(db: &dyn Database) {
        let sid1 = db.create_subnet(&subnet(11)).await.unwrap();
        let sid2 = db.create_subnet(&subnet(12)).await.unwrap();
        db.create_range(&range(sid1, 11)).await.unwrap();
        db.create_range(&range(sid2, 12)).await.unwrap();

        let all = db.list_ranges(None).await.unwrap();
        assert!(all.len() >= 2);
    }

    pub async fn test_delete_range(db: &dyn Database) {
        let sid = db.create_subnet(&subnet(13)).await.unwrap();
        let range_id = db.create_range(&range(sid, 13)).await.unwrap();

        db.delete_range(range_id).await.unwrap();

        let ranges = db.list_ranges(Some(sid)).await.unwrap();
        assert!(ranges.iter().all(|r| r.id != Some(range_id)));
    }

    // --- Static IP tests ---

    pub async fn test_create_and_list_static_ip(db: &dyn Database) {
        let sid = db.create_subnet(&subnet(20)).await.unwrap();
        let id = db
            .create_static_ip(&static_ip(sid, "01", 20))
            .await
            .unwrap();
        assert!(id > 0);

        let ips = db.list_static_ips(Some(sid)).await.unwrap();
        assert_eq!(ips.len(), 1);
        assert_eq!(ips[0].mac_address, "aa:bb:cc:dd:ee:01");
        assert_eq!(ips[0].ip_address, Ipv4Addr::new(10, 0, 20, 50));
    }

    pub async fn test_get_static_ip_by_mac(db: &dyn Database) {
        let sid = db.create_subnet(&subnet(21)).await.unwrap();
        db.create_static_ip(&static_ip(sid, "02", 21))
            .await
            .unwrap();

        let result = db.get_static_ip_by_mac("aa:bb:cc:dd:ee:02").await.unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().ip_address, Ipv4Addr::new(10, 0, 21, 50));
    }

    pub async fn test_get_static_ip_by_mac_not_found(db: &dyn Database) {
        assert!(db
            .get_static_ip_by_mac("00:00:00:00:00:00")
            .await
            .unwrap()
            .is_none());
    }

    pub async fn test_delete_static_ip(db: &dyn Database) {
        let sid = db.create_subnet(&subnet(22)).await.unwrap();
        let id = db
            .create_static_ip(&static_ip(sid, "03", 22))
            .await
            .unwrap();

        db.delete_static_ip(id).await.unwrap();

        let ips = db.list_static_ips(Some(sid)).await.unwrap();
        assert!(ips.iter().all(|s| s.id != Some(id)));
    }

    // --- Lease tests ---

    pub async fn test_create_and_get_active_lease(db: &dyn Database) {
        let sid = db.create_subnet(&subnet(30)).await.unwrap();
        let id = db.create_lease(&active_lease(sid, "10", 30)).await.unwrap();
        assert!(id > 0);

        let lease = db
            .get_active_lease("aa:bb:cc:dd:ee:10")
            .await
            .unwrap()
            .expect("lease not found");
        assert_eq!(lease.mac_address, "aa:bb:cc:dd:ee:10");
        assert_eq!(lease.ip_address, Ipv4Addr::new(10, 0, 30, 80));
        assert!(lease.active);
    }

    pub async fn test_list_active_leases(db: &dyn Database) {
        let sid = db.create_subnet(&subnet(31)).await.unwrap();
        db.create_lease(&active_lease(sid, "11", 31)).await.unwrap();

        let leases = db.list_active_leases().await.unwrap();
        assert!(!leases.is_empty());
    }

    pub async fn test_expire_lease(db: &dyn Database) {
        let sid = db.create_subnet(&subnet(32)).await.unwrap();
        let id = db.create_lease(&active_lease(sid, "12", 32)).await.unwrap();

        db.expire_lease(id).await.unwrap();

        assert!(db
            .get_active_lease("aa:bb:cc:dd:ee:12")
            .await
            .unwrap()
            .is_none());
    }

    pub async fn test_expired_lease_not_returned(db: &dyn Database) {
        let sid = db.create_subnet(&subnet(33)).await.unwrap();
        let now = chrono::Utc::now().timestamp();
        let expired = Lease {
            id: None,
            subnet_id: sid,
            mac_address: "aa:bb:cc:dd:ee:13".to_string(),
            ip_address: Ipv4Addr::new(10, 0, 33, 90),
            lease_start: now - 7200,
            lease_end: now - 3600,
            hostname: None,
            active: true,
        };
        db.create_lease(&expired).await.unwrap();

        assert!(db
            .get_active_lease("aa:bb:cc:dd:ee:13")
            .await
            .unwrap()
            .is_none());
    }

    // --- IA Prefix tests ---

    pub async fn test_create_and_get_ia_prefix(db: &dyn Database) {
        let id = db.create_ia_prefix(&ia_prefix("iap0", 1)).await.unwrap();
        assert!(id > 0);

        let prefix = db
            .get_ia_prefix(id)
            .await
            .unwrap()
            .expect("prefix not found");
        assert_eq!(prefix.id, Some(id));
        assert_eq!(prefix.interface, "iap0");
        assert_eq!(prefix.prefix_len, 64);
        assert!(prefix.enabled);
    }

    pub async fn test_list_ia_prefixes_by_interface(db: &dyn Database) {
        db.create_ia_prefix(&ia_prefix("iap1", 2)).await.unwrap();
        db.create_ia_prefix(&ia_prefix("iap2", 3)).await.unwrap();

        let iap1 = db.list_ia_prefixes(Some("iap1")).await.unwrap();
        assert!(iap1.iter().all(|p| p.interface == "iap1"));

        let all = db.list_ia_prefixes(None).await.unwrap();
        assert!(all.len() >= 2);
    }

    pub async fn test_list_enabled_ia_prefixes(db: &dyn Database) {
        db.create_ia_prefix(&ia_prefix("iap3", 4)).await.unwrap();

        let mut disabled = ia_prefix("iap4", 5);
        disabled.enabled = false;
        db.create_ia_prefix(&disabled).await.unwrap();

        let enabled = db.list_enabled_ia_prefixes().await.unwrap();
        assert!(enabled.iter().all(|p| p.enabled));
    }

    pub async fn test_update_ia_prefix(db: &dyn Database) {
        let id = db.create_ia_prefix(&ia_prefix("iap5", 6)).await.unwrap();

        let mut updated = ia_prefix("iap5", 6);
        updated.prefix_len = 48;
        updated.enabled = false;
        db.update_ia_prefix(id, &updated).await.unwrap();

        let prefix = db
            .get_ia_prefix(id)
            .await
            .unwrap()
            .expect("prefix not found");
        assert_eq!(prefix.prefix_len, 48);
        assert!(!prefix.enabled);
    }

    pub async fn test_delete_ia_prefix(db: &dyn Database) {
        let id = db.create_ia_prefix(&ia_prefix("iap6", 7)).await.unwrap();
        db.delete_ia_prefix(id).await.unwrap();

        assert!(db.get_ia_prefix(id).await.unwrap().is_none());
    }

    // --- Token tests ---

    pub async fn test_create_and_list_tokens(db: &dyn Database) {
        let id = db
            .create_token("test-token", "hash_tok1", "salt_tok1")
            .await
            .unwrap();
        assert!(id > 0);

        let tokens = db.list_tokens().await.unwrap();
        assert!(tokens.iter().any(|(h, _)| h == "hash_tok1"));
    }

    pub async fn test_list_api_tokens(db: &dyn Database) {
        db.create_token("my-token", "hash_tok2", "salt_tok2")
            .await
            .unwrap();

        let tokens = db.list_api_tokens().await.unwrap();
        assert!(tokens.iter().any(|t| t.name == "my-token"));
    }

    pub async fn test_delete_token(db: &dyn Database) {
        let id = db
            .create_token("to-delete", "hash_tok3", "salt_tok3")
            .await
            .unwrap();
        db.delete_token(id).await.unwrap();

        let tokens = db.list_tokens().await.unwrap();
        assert!(!tokens.iter().any(|(h, _)| h == "hash_tok3"));
    }

    pub async fn test_toggle_token(db: &dyn Database) {
        let id = db
            .create_token("toggle-me", "hash_tok4", "salt_tok4")
            .await
            .unwrap();

        db.toggle_token(id, false).await.unwrap();
        let tokens = db.list_tokens().await.unwrap();
        assert!(!tokens.iter().any(|(h, _)| h == "hash_tok4"));

        db.toggle_token(id, true).await.unwrap();
        let tokens = db.list_tokens().await.unwrap();
        assert!(tokens.iter().any(|(h, _)| h == "hash_tok4"));
    }

    pub async fn test_update_token_last_used(db: &dyn Database) {
        db.create_token("last-used", "hash_tok5", "salt_tok5")
            .await
            .unwrap();
        db.update_token_last_used("hash_tok5").await.unwrap();
    }

    /// Run the full test suite sequentially against one Database instance.
    /// All test functions use distinct data (network octets, MACs, etc.)
    /// so they can safely share the same database.
    pub async fn run_all(db: &dyn Database) {
        test_create_and_get_subnet(db).await;
        test_list_subnets(db).await;
        test_list_active_subnets(db).await;
        test_update_subnet(db).await;
        test_delete_subnet(db).await;
        test_get_subnet_not_found(db).await;

        test_create_and_list_range(db).await;
        test_list_ranges_all(db).await;
        test_delete_range(db).await;

        test_create_and_list_static_ip(db).await;
        test_get_static_ip_by_mac(db).await;
        test_get_static_ip_by_mac_not_found(db).await;
        test_delete_static_ip(db).await;

        test_create_and_get_active_lease(db).await;
        test_list_active_leases(db).await;
        test_expire_lease(db).await;
        test_expired_lease_not_returned(db).await;

        test_create_and_get_ia_prefix(db).await;
        test_list_ia_prefixes_by_interface(db).await;
        test_list_enabled_ia_prefixes(db).await;
        test_update_ia_prefix(db).await;
        test_delete_ia_prefix(db).await;

        test_create_and_list_tokens(db).await;
        test_list_api_tokens(db).await;
        test_delete_token(db).await;
        test_toggle_token(db).await;
        test_update_token_last_used(db).await;
    }
}
