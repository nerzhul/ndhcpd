use serde::{Deserialize, Serialize};
use std::net::{Ipv4Addr, Ipv6Addr};
use utoipa::ToSchema;

/// A subnet configuration
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Subnet {
    pub id: Option<i64>,

    /// Network address (e.g., 192.168.1.0)
    #[schema(value_type = String)]
    pub network: Ipv4Addr,

    /// Subnet mask (e.g., 24 for /24)
    pub netmask: u8,

    /// Gateway/router address
    #[schema(value_type = String)]
    pub gateway: Ipv4Addr,

    /// DNS servers (comma-separated in DB)
    #[schema(value_type = Vec<String>)]
    pub dns_servers: Vec<Ipv4Addr>,

    /// Domain name
    pub domain_name: Option<String>,

    /// Whether this subnet is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

/// A dynamic IP range within a subnet
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DynamicRange {
    pub id: Option<i64>,

    /// Foreign key to subnet
    pub subnet_id: i64,

    /// Start of the range (e.g., 192.168.1.100)
    #[schema(value_type = String)]
    pub range_start: Ipv4Addr,

    /// End of the range (e.g., 192.168.1.200)
    #[schema(value_type = String)]
    pub range_end: Ipv4Addr,

    /// Whether this range is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// A static IP assignment
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct StaticIP {
    /// Foreign key to subnet
    pub subnet_id: i64,

    /// MAC address (format: XX:XX:XX:XX:XX:XX)
    pub mac_address: String,

    /// Assigned IP address
    #[schema(value_type = String)]
    pub ip_address: Ipv4Addr,

    /// Optional hostname
    pub hostname: Option<String>,
}

/// A DHCP lease
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Lease {
    pub id: Option<i64>,

    /// Foreign key to subnet
    pub subnet_id: i64,

    /// MAC address of the client
    pub mac_address: String,

    /// Leased IP address
    #[schema(value_type = String)]
    pub ip_address: Ipv4Addr,

    /// Lease start time (Unix timestamp)
    pub lease_start: i64,

    /// Lease end time (Unix timestamp)
    pub lease_end: i64,

    /// Optional hostname
    pub hostname: Option<String>,
}

// Helper functions for converting between String and Ipv4Addr for sqlx
impl Subnet {
    pub fn dns_servers_to_string(&self) -> String {
        self.dns_servers
            .iter()
            .map(|ip| ip.to_string())
            .collect::<Vec<_>>()
            .join(",")
    }

    pub fn dns_servers_from_string(s: &str) -> Vec<Ipv4Addr> {
        s.split(',')
            .filter_map(|ip| ip.trim().parse().ok())
            .collect()
    }
}

/// An API token for authentication
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ApiToken {
    pub id: Option<i64>,

    /// Descriptive name for the token
    pub name: String,

    /// Token hash (not exposed to client after creation)
    #[serde(skip_serializing)]
    pub token_hash: Option<String>,

    /// Salt for token (not exposed to client)
    #[serde(skip_serializing)]
    pub salt: Option<String>,

    /// Creation timestamp
    pub created_at: Option<i64>,

    /// Last used timestamp
    pub last_used_at: Option<i64>,

    /// Whether the token is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// The actual token value (only returned on creation)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
}

/// Request to create a new API token
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateTokenRequest {
    /// Descriptive name for the token
    pub name: String,
}

/// Response when creating a new API token
#[derive(Debug, Serialize, ToSchema)]
pub struct CreateTokenResponse {
    /// The token ID
    pub id: i64,

    /// The token name
    pub name: String,

    /// The actual token value (only shown once)
    pub token: String,
}

/// An IPv6 prefix for Router Advertisement (SLAAC)
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct IAPrefix {
    pub id: Option<i64>,

    /// Network interface name (e.g., "eth0", "br0")
    pub interface: String,

    /// IPv6 prefix network address (e.g., 2001:db8::)
    #[schema(value_type = String)]
    pub prefix: Ipv6Addr,

    /// Prefix length (e.g., 64 for /64)
    pub prefix_len: u8,

    /// Preferred lifetime in seconds
    pub preferred_lifetime: u32,

    /// Valid lifetime in seconds
    pub valid_lifetime: u32,

    /// DNS servers for RDNSS option (RFC 6106)
    #[schema(value_type = Vec<String>)]
    pub dns_servers: Vec<Ipv6Addr>,

    /// DNS recursive lookup lifetime in seconds
    pub dns_lifetime: u32,

    /// Whether this prefix is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl IAPrefix {
    /// Convert DNS servers to comma-separated string for DB storage
    pub fn dns_servers_to_string(&self) -> String {
        self.dns_servers
            .iter()
            .map(|ip| ip.to_string())
            .collect::<Vec<_>>()
            .join(",")
    }

    /// Parse DNS servers from comma-separated string
    pub fn dns_servers_from_string(s: &str) -> Vec<Ipv6Addr> {
        s.split(',')
            .filter_map(|ip| ip.trim().parse().ok())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv6Addr;

    #[test]
    fn test_ia_prefix_dns_servers_to_string_single() {
        let prefix = IAPrefix {
            id: None,
            interface: "eth0".to_string(),
            prefix: Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 0),
            prefix_len: 64,
            preferred_lifetime: 86400,
            valid_lifetime: 2592000,
            dns_servers: vec![Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1)],
            dns_lifetime: 86400,
            enabled: true,
        };

        assert_eq!(prefix.dns_servers_to_string(), "2001:db8::1");
    }

    #[test]
    fn test_ia_prefix_dns_servers_to_string_multiple() {
        let prefix = IAPrefix {
            id: None,
            interface: "eth0".to_string(),
            prefix: Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 0),
            prefix_len: 64,
            preferred_lifetime: 86400,
            valid_lifetime: 2592000,
            dns_servers: vec![
                Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1),
                Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 2),
            ],
            dns_lifetime: 86400,
            enabled: true,
        };

        assert_eq!(prefix.dns_servers_to_string(), "2001:db8::1,2001:db8::2");
    }

    #[test]
    fn test_ia_prefix_dns_servers_to_string_empty() {
        let prefix = IAPrefix {
            id: None,
            interface: "eth0".to_string(),
            prefix: Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 0),
            prefix_len: 64,
            preferred_lifetime: 86400,
            valid_lifetime: 2592000,
            dns_servers: vec![],
            dns_lifetime: 86400,
            enabled: true,
        };

        assert_eq!(prefix.dns_servers_to_string(), "");
    }

    #[test]
    fn test_ia_prefix_dns_servers_from_string_single() {
        let result = IAPrefix::dns_servers_from_string("2001:db8::1");

        assert_eq!(result.len(), 1);
        assert_eq!(result[0], Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1));
    }

    #[test]
    fn test_ia_prefix_dns_servers_from_string_multiple() {
        let result = IAPrefix::dns_servers_from_string("2001:db8::1,2001:db8::2");

        assert_eq!(result.len(), 2);
        assert_eq!(result[0], Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1));
        assert_eq!(result[1], Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 2));
    }

    #[test]
    fn test_ia_prefix_dns_servers_from_string_empty() {
        let result = IAPrefix::dns_servers_from_string("");

        assert!(result.is_empty());
    }

    #[test]
    fn test_ia_prefix_dns_servers_from_string_with_spaces() {
        let result = IAPrefix::dns_servers_from_string("2001:db8::1, 2001:db8::2");

        assert_eq!(result.len(), 2);
        assert_eq!(result[0], Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1));
        assert_eq!(result[1], Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 2));
    }

    #[test]
    fn test_ia_prefix_dns_servers_from_string_invalid_ignored() {
        let result = IAPrefix::dns_servers_from_string("2001:db8::1,invalid,2001:db8::2");

        assert_eq!(result.len(), 2);
        assert_eq!(result[0], Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1));
        assert_eq!(result[1], Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 2));
    }

    #[test]
    fn test_ia_prefix_roundtrip_serialization() {
        let prefix = IAPrefix {
            id: Some(1),
            interface: "eth0".to_string(),
            prefix: Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 0),
            prefix_len: 64,
            preferred_lifetime: 86400,
            valid_lifetime: 2592000,
            dns_servers: vec![
                Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1),
                Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 2),
            ],
            dns_lifetime: 3600,
            enabled: true,
        };

        let json = serde_json::to_string(&prefix).unwrap();
        let deserialized: IAPrefix = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id, prefix.id);
        assert_eq!(deserialized.interface, prefix.interface);
        assert_eq!(deserialized.prefix, prefix.prefix);
        assert_eq!(deserialized.prefix_len, prefix.prefix_len);
        assert_eq!(deserialized.preferred_lifetime, prefix.preferred_lifetime);
        assert_eq!(deserialized.valid_lifetime, prefix.valid_lifetime);
        assert_eq!(deserialized.dns_servers, prefix.dns_servers);
        assert_eq!(deserialized.dns_lifetime, prefix.dns_lifetime);
        assert_eq!(deserialized.enabled, prefix.enabled);
    }

    #[test]
    fn test_ia_prefix_default_enabled() {
        let json = r#"{
            "interface": "eth0",
            "prefix": "2001:db8::",
            "prefix_len": 64,
            "preferred_lifetime": 86400,
            "valid_lifetime": 2592000,
            "dns_servers": [],
            "dns_lifetime": 86400
        }"#;

        let prefix: IAPrefix = serde_json::from_str(json).unwrap();

        assert!(prefix.enabled);
    }

    #[test]
    fn test_subnet_dns_servers_to_string() {
        let subnet = Subnet {
            id: None,
            network: Ipv4Addr::new(192, 168, 1, 0),
            netmask: 24,
            gateway: Ipv4Addr::new(192, 168, 1, 1),
            dns_servers: vec![Ipv4Addr::new(8, 8, 8, 8), Ipv4Addr::new(1, 1, 1, 1)],
            domain_name: Some("local".to_string()),
            enabled: true,
        };

        assert_eq!(subnet.dns_servers_to_string(), "8.8.8.8,1.1.1.1");
    }

    #[test]
    fn test_subnet_dns_servers_from_string() {
        let result = Subnet::dns_servers_from_string("8.8.8.8,1.1.1.1");

        assert_eq!(result.len(), 2);
        assert_eq!(result[0], Ipv4Addr::new(8, 8, 8, 8));
        assert_eq!(result[1], Ipv4Addr::new(1, 1, 1, 1));
    }
}
