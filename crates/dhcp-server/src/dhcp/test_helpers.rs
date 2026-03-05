#[cfg(test)]
use super::*;
#[cfg(test)]
use crate::config::{ApiConfig, Config, DhcpConfig};
#[cfg(test)]
use crate::models::Subnet;
#[cfg(test)]
use dhcp_proto::MacAddress;
#[cfg(test)]
use std::net::Ipv4Addr;

#[cfg(test)]
pub fn create_test_config() -> Config {
    Config {
        listen_interfaces: vec!["lo".to_string()],
        database_path: ":memory:".to_string(),
        api: ApiConfig {
            listen_address: "127.0.0.1".to_string(),
            port: 8080,
            unix_socket: None,
            require_authentication: Some(false),
        },
        dhcp: DhcpConfig {
            default_lease_time: 86400,
            max_lease_time: 604800,
        },
        ra: None,
    }
}

#[cfg(test)]
pub fn create_test_subnet() -> Subnet {
    Subnet {
        id: Some(1),
        network: Ipv4Addr::new(192, 168, 1, 0),
        netmask: 24,
        gateway: Ipv4Addr::new(192, 168, 1, 1),
        dns_servers: vec![Ipv4Addr::new(8, 8, 8, 8), Ipv4Addr::new(8, 8, 4, 4)],
        domain_name: Some("test.local".to_string()),
        enabled: true,
    }
}

/// Create a DHCP DISCOVER packet
#[cfg(test)]
pub fn create_discover_packet(mac: &str) -> DhcpPacket {
    let mut packet = DhcpPacket::new();
    packet.op = 1; // BOOTREQUEST
    packet.xid = 12345;
    packet.chaddr = MacAddress::from_string(mac).unwrap();
    packet
        .options
        .push(DhcpOption::MessageType(MessageType::Discover));
    packet
}

/// Create a DHCP REQUEST packet
#[cfg(test)]
pub fn create_request_packet(mac: &str, requested_ip: Ipv4Addr) -> DhcpPacket {
    let mut packet = DhcpPacket::new();
    packet.op = 1; // BOOTREQUEST
    packet.xid = 67890;
    packet.chaddr = MacAddress::from_string(mac).unwrap();
    packet
        .options
        .push(DhcpOption::MessageType(MessageType::Request));
    packet
        .options
        .push(DhcpOption::RequestedIpAddress(requested_ip));
    packet
}

/// Create a DHCP RELEASE packet
#[cfg(test)]
pub fn create_release_packet(mac: &str) -> DhcpPacket {
    let mut packet = DhcpPacket::new();
    packet.op = 1; // BOOTREQUEST
    packet.xid = 11111;
    packet.chaddr = MacAddress::from_string(mac).unwrap();
    packet
        .options
        .push(DhcpOption::MessageType(MessageType::Release));
    packet
}

/// Create a DHCP INFORM packet
#[cfg(test)]
pub fn create_inform_packet(mac: &str) -> DhcpPacket {
    let mut packet = DhcpPacket::new();
    packet.op = 1; // BOOTREQUEST
    packet.xid = 99999;
    packet.chaddr = MacAddress::from_string(mac).unwrap();
    packet
        .options
        .push(DhcpOption::MessageType(MessageType::Inform));
    packet
}
