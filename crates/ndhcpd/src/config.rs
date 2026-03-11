use serde::{Deserialize, Serialize};

/// Configuration structure loaded from YAML
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Network interfaces the DHCP server should listen on
    pub listen_interfaces: Vec<String>,

    /// Database file path
    #[serde(default = "default_db_path")]
    pub database_path: String,

    /// API server configuration
    pub api: ApiConfig,

    /// DHCP server configuration
    pub dhcp: DhcpConfig,

    /// Router Advertisement (IPv6) configuration
    #[serde(default)]
    pub ra: Option<RaConfig>,
}

fn default_db_path() -> String {
    #[cfg(target_os = "freebsd")]
    return "/var/db/ndhcpd.db".to_string();
    #[cfg(not(target_os = "freebsd"))]
    return "/var/lib/ndhcpd/dhcp.db".to_string();
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    /// API listening address
    #[serde(default = "default_api_address")]
    pub listen_address: String,

    /// API listening port
    #[serde(default = "default_api_port")]
    pub port: u16,

    /// Unix socket path (optional, for local communication)
    #[serde(default = "default_unix_socket")]
    pub unix_socket: Option<String>,

    /// Require token authentication for TCP API (not Unix socket)
    #[serde(default)]
    pub require_authentication: Option<bool>,
}

fn default_api_address() -> String {
    "127.0.0.1".to_string()
}

fn default_api_port() -> u16 {
    8080
}

fn default_unix_socket() -> Option<String> {
    Some("/var/run/ndhcpd.sock".to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DhcpConfig {
    /// Default lease time in seconds
    #[serde(default = "default_lease_time")]
    pub default_lease_time: u32,

    /// Maximum lease time in seconds
    #[serde(default = "default_max_lease_time")]
    pub max_lease_time: u32,
}

fn default_lease_time() -> u32 {
    86400 // 24 hours
}

fn default_max_lease_time() -> u32 {
    604800 // 7 days
}

/// Router Advertisement (IPv6) configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaConfig {
    /// Default preferred lifetime in seconds
    #[serde(default = "default_ra_preferred_lifetime")]
    pub default_preferred_lifetime: u32,

    /// Default valid lifetime in seconds
    #[serde(default = "default_ra_valid_lifetime")]
    pub default_valid_lifetime: u32,

    /// Default DNS lifetime in seconds (RDNSS option)
    #[serde(default = "default_ra_dns_lifetime")]
    pub default_dns_lifetime: u32,
}

fn default_ra_preferred_lifetime() -> u32 {
    86400 // 24 hours
}

fn default_ra_valid_lifetime() -> u32 {
    2592000 // 30 days
}

fn default_ra_dns_lifetime() -> u32 {
    86400 // 24 hours
}

impl Default for RaConfig {
    fn default() -> Self {
        Self {
            default_preferred_lifetime: default_ra_preferred_lifetime(),
            default_valid_lifetime: default_ra_valid_lifetime(),
            default_dns_lifetime: default_ra_dns_lifetime(),
        }
    }
}

impl Config {
    /// Load configuration from a YAML file
    pub fn from_file(path: &str) -> anyhow::Result<Self> {
        let contents = std::fs::read_to_string(path)?;
        let config = serde_yaml::from_str(&contents)?;
        Ok(config)
    }

    /// Save configuration to a YAML file
    pub fn to_file(&self, path: &str) -> anyhow::Result<()> {
        let yaml = serde_yaml::to_string(self)?;
        std::fs::write(path, yaml)?;
        Ok(())
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            listen_interfaces: vec!["eth0".to_string()],
            database_path: default_db_path(),
            api: ApiConfig {
                listen_address: default_api_address(),
                port: default_api_port(),
                unix_socket: default_unix_socket(),
                require_authentication: Some(false),
            },
            dhcp: DhcpConfig {
                default_lease_time: default_lease_time(),
                max_lease_time: default_max_lease_time(),
            },
            ra: None,
        }
    }
}
