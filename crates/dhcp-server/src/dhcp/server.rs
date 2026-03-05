use std::collections::HashSet;
use std::ffi::CString;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::os::unix::io::AsRawFd;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use super::packet::{DhcpOption, DhcpPacket, MessageType};
use crate::config::Config;
use crate::db::{Database, DynDatabase};

const DHCP_SERVER_PORT: u16 = 67;
const DHCP_CLIENT_PORT: u16 = 68;

pub struct DhcpServer {
    config: Arc<Config>,
    db: DynDatabase,
}

impl DhcpServer {
    pub fn new(config: Arc<Config>, db: DynDatabase) -> Self {
        Self { config, db }
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        info!("Starting DHCP server");

        for interface in &self.config.listen_interfaces {
            info!("Binding to interface {}", interface);

            // Clone Arc references for the spawned task
            let config = Arc::clone(&self.config);
            let db = Arc::clone(&self.db);
            let interface = interface.clone();

            tokio::spawn(async move {
                if let Err(e) = Self::listen_loop(interface.clone(), config, db).await {
                    error!("DHCP listener error on interface {}: {}", interface, e);
                }
            });
        }

        // Keep running
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
        }
    }

    async fn listen_loop(
        interface: String,
        config: Arc<Config>,
        db: DynDatabase,
    ) -> anyhow::Result<()> {
        let addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), DHCP_SERVER_PORT);
        let socket = UdpSocket::bind(addr)?;
        socket.set_broadcast(true)?;

        // Bind the socket to the specific network interface so only packets
        // arriving on that interface are received (SO_BINDTODEVICE).
        let iface_cstr = CString::new(interface.as_str())?;
        let ret = unsafe {
            libc::setsockopt(
                socket.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_BINDTODEVICE,
                iface_cstr.as_ptr() as *const libc::c_void,
                iface_cstr.as_bytes_with_nul().len() as libc::socklen_t,
            )
        };
        if ret != 0 {
            return Err(anyhow::anyhow!(
                "Failed to bind socket to interface {}: {}",
                interface,
                std::io::Error::last_os_error()
            ));
        }

        info!(
            "DHCP server listening on interface {} (0.0.0.0:{})",
            interface, DHCP_SERVER_PORT
        );

        let mut buf = vec![0u8; 1024];

        loop {
            let (len, src) = socket.recv_from(&mut buf)?;
            debug!("Received {} bytes from {}", len, src);

            let packet_data = &buf[..len];

            // Parse packet
            let packet = match DhcpPacket::parse(packet_data) {
                Ok(p) => p,
                Err(e) => {
                    warn!("Failed to parse DHCP packet: {}", e);
                    continue;
                }
            };

            // Handle packet
            let response = Self::handle_packet(&packet, &config, &*db).await;

            if let Some(response_packet) = response {
                let response_bytes = response_packet.to_bytes();
                let broadcast_addr =
                    SocketAddr::new(Ipv4Addr::new(255, 255, 255, 255).into(), DHCP_CLIENT_PORT);

                if let Err(e) = socket.send_to(&response_bytes, broadcast_addr) {
                    warn!("Failed to send DHCP response: {}", e);
                }
            }
        }
    }

    async fn handle_packet(
        packet: &DhcpPacket,
        config: &Config,
        db: &dyn Database,
    ) -> Option<DhcpPacket> {
        let msg_type = packet.get_message_type()?;
        let mac = packet.chaddr.to_string();

        match msg_type {
            MessageType::Discover => {
                info!("DHCP DISCOVER from {}", mac);
                Self::handle_discover(packet, config, db).await
            }
            MessageType::Request => {
                info!("DHCP REQUEST from {}", mac);
                Self::handle_request(packet, config, db).await
            }
            MessageType::Release => {
                info!("DHCP RELEASE from {}", mac);
                Self::handle_release(packet, db).await;
                None
            }
            MessageType::Inform => {
                info!("DHCP INFORM from {}", mac);
                None // Not implemented yet
            }
            _ => {
                debug!("Unhandled DHCP message type: {:?}", msg_type);
                None
            }
        }
    }

    async fn handle_discover(
        packet: &DhcpPacket,
        config: &Config,
        db: &dyn Database,
    ) -> Option<DhcpPacket> {
        let mac = packet.chaddr.to_string();

        // Check for static IP assignment
        if let Ok(Some(static_ip)) = db.get_static_ip_by_mac(&mac).await {
            let subnet = db.get_subnet(static_ip.subnet_id).await.ok()??;

            return Some(Self::create_offer(
                packet,
                static_ip.ip_address,
                &subnet,
                config,
            ));
        }

        // Check for existing lease
        if let Ok(Some(lease)) = db.get_active_lease(&mac).await {
            let subnet = db.get_subnet(lease.subnet_id).await.ok()??;

            return Some(Self::create_offer(
                packet,
                lease.ip_address,
                &subnet,
                config,
            ));
        }

        // Allocate a new IP from an enabled dynamic range
        let ranges = match db.list_ranges(None).await {
            Ok(r) => r,
            Err(e) => {
                error!("Failed to list dynamic ranges: {}", e);
                return None;
            }
        };

        // Build the set of IPs already in use to avoid double-allocation
        let leased_ips: HashSet<Ipv4Addr> = match db.list_active_leases().await {
            Ok(leases) => leases.into_iter().map(|l| l.ip_address).collect(),
            Err(e) => {
                error!("Failed to list active leases: {}", e);
                return None;
            }
        };

        for range in ranges.iter().filter(|r| r.enabled) {
            let start = u32::from(range.range_start);
            let end = u32::from(range.range_end);

            for ip_u32 in start..=end {
                let candidate = Ipv4Addr::from(ip_u32);
                if !leased_ips.contains(&candidate) {
                    let subnet = match db.get_subnet(range.subnet_id).await {
                        Ok(Some(s)) => s,
                        _ => continue,
                    };
                    debug!("Offering dynamic IP {} to {}", candidate, mac);
                    return Some(Self::create_offer(packet, candidate, &subnet, config));
                }
            }
        }

        warn!("No free IP available for DISCOVER from {}", mac);
        None
    }

    async fn handle_request(
        packet: &DhcpPacket,
        config: &Config,
        db: &dyn Database,
    ) -> Option<DhcpPacket> {
        let mac = packet.chaddr.to_string();

        // Extract requested IP: from option 50 (new request) or ciaddr (renewal)
        let requested_ip = packet
            .options
            .iter()
            .find_map(|opt| {
                if let DhcpOption::RequestedIpAddress(ip) = opt {
                    Some(*ip)
                } else {
                    None
                }
            })
            .or_else(|| {
                if packet.ciaddr != Ipv4Addr::UNSPECIFIED {
                    Some(packet.ciaddr)
                } else {
                    None
                }
            })?;

        // Extract optional hostname sent by the client
        let hostname = packet.options.iter().find_map(|opt| {
            if let DhcpOption::Hostname(h) = opt {
                Some(h.clone())
            } else {
                None
            }
        });

        // Check for static IP assignment
        if let Ok(Some(static_ip)) = db.get_static_ip_by_mac(&mac).await {
            if static_ip.ip_address == requested_ip {
                let subnet = db.get_subnet(static_ip.subnet_id).await.ok()??;
                return Some(Self::create_ack(packet, requested_ip, &subnet, config));
            }
            // Static IP exists but client requested a different one: NAK
            warn!(
                "Client {} requested {} but has static assignment {}",
                mac, requested_ip, static_ip.ip_address
            );
            return None;
        }

        // Check if the requested IP falls within an enabled dynamic range
        let ranges = match db.list_ranges(None).await {
            Ok(r) => r,
            Err(e) => {
                error!("Failed to list dynamic ranges: {}", e);
                return None;
            }
        };

        let matching_range = ranges.iter().find(|r| {
            r.enabled
                && u32::from(r.range_start) <= u32::from(requested_ip)
                && u32::from(requested_ip) <= u32::from(r.range_end)
        })?;

        // Verify the IP is not already leased by a different MAC
        let active_leases = match db.list_active_leases().await {
            Ok(l) => l,
            Err(e) => {
                error!("Failed to list active leases: {}", e);
                return None;
            }
        };

        if let Some(existing) = active_leases.iter().find(|l| l.ip_address == requested_ip) {
            if existing.mac_address != mac {
                warn!(
                    "Client {} requested {} already leased to {}",
                    mac, requested_ip, existing.mac_address
                );
                return None;
            }
            // Same MAC renewing: expire old lease before creating a new one
            if let Some(id) = existing.id {
                let _ = db.expire_lease(id).await;
            }
        }

        // Retrieve the subnet for this range
        let subnet = match db.get_subnet(matching_range.subnet_id).await {
            Ok(Some(s)) => s,
            _ => {
                error!(
                    "Subnet {} not found for dynamic range",
                    matching_range.subnet_id
                );
                return None;
            }
        };

        // Create the lease
        let now = chrono::Utc::now().timestamp();
        let lease = crate::models::Lease {
            id: None,
            subnet_id: matching_range.subnet_id,
            mac_address: mac.clone(),
            ip_address: requested_ip,
            lease_start: now,
            lease_end: now + config.dhcp.default_lease_time as i64,
            hostname,
            active: true,
        };

        if let Err(e) = db.create_lease(&lease).await {
            error!("Failed to create lease for {}: {}", mac, e);
            return None;
        }

        info!(
            "Dynamic lease created: {} -> {} (subnet {})",
            mac, requested_ip, subnet.network
        );
        Some(Self::create_ack(packet, requested_ip, &subnet, config))
    }

    async fn handle_release(packet: &DhcpPacket, db: &dyn Database) {
        let mac = packet.chaddr.to_string();

        if let Ok(Some(lease)) = db.get_active_lease(&mac).await {
            if let Some(id) = lease.id {
                let _ = db.expire_lease(id).await;
            }
        }
    }

    fn create_offer(
        request: &DhcpPacket,
        offered_ip: Ipv4Addr,
        subnet: &crate::models::Subnet,
        config: &Config,
    ) -> DhcpPacket {
        let mut packet = DhcpPacket::new();
        packet.op = 2; // BOOTREPLY
        packet.xid = request.xid;
        packet.yiaddr = offered_ip;
        packet.chaddr = request.chaddr.clone();
        packet.siaddr = subnet.gateway;

        packet
            .options
            .push(DhcpOption::MessageType(MessageType::Offer));
        packet
            .options
            .push(DhcpOption::ServerIdentifier(subnet.gateway));
        packet
            .options
            .push(DhcpOption::LeaseTime(config.dhcp.default_lease_time));
        packet
            .options
            .push(DhcpOption::SubnetMask(Self::netmask_from_prefix(
                subnet.netmask,
            )));
        packet
            .options
            .push(DhcpOption::Router(vec![subnet.gateway]));
        packet
            .options
            .push(DhcpOption::DnsServer(subnet.dns_servers.clone()));

        if let Some(domain) = &subnet.domain_name {
            packet.options.push(DhcpOption::DomainName(domain.clone()));
        }

        packet
    }

    fn create_ack(
        request: &DhcpPacket,
        assigned_ip: Ipv4Addr,
        subnet: &crate::models::Subnet,
        config: &Config,
    ) -> DhcpPacket {
        let mut packet = DhcpPacket::new();
        packet.op = 2; // BOOTREPLY
        packet.xid = request.xid;
        packet.yiaddr = assigned_ip;
        packet.chaddr = request.chaddr.clone();
        packet.siaddr = subnet.gateway;

        packet
            .options
            .push(DhcpOption::MessageType(MessageType::Ack));
        packet
            .options
            .push(DhcpOption::ServerIdentifier(subnet.gateway));
        packet
            .options
            .push(DhcpOption::LeaseTime(config.dhcp.default_lease_time));
        packet
            .options
            .push(DhcpOption::RenewalTime(config.dhcp.default_lease_time / 2));
        packet.options.push(DhcpOption::RebindingTime(
            config.dhcp.default_lease_time * 7 / 8,
        ));
        packet
            .options
            .push(DhcpOption::SubnetMask(Self::netmask_from_prefix(
                subnet.netmask,
            )));
        packet
            .options
            .push(DhcpOption::Router(vec![subnet.gateway]));
        packet
            .options
            .push(DhcpOption::DnsServer(subnet.dns_servers.clone()));

        if let Some(domain) = &subnet.domain_name {
            packet.options.push(DhcpOption::DomainName(domain.clone()));
        }

        packet
    }

    fn netmask_from_prefix(prefix: u8) -> Ipv4Addr {
        let mask = if prefix == 0 {
            0u32
        } else {
            !0u32 << (32 - prefix)
        };
        Ipv4Addr::from(mask)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::InMemoryDatabase;
    use crate::dhcp::test_helpers::*;
    use crate::models::{Lease, StaticIP};

    #[tokio::test]
    async fn test_handle_discover_with_static_ip() {
        let config = create_test_config();
        let db = InMemoryDatabase::new();

        // Create subnet
        let subnet = create_test_subnet();
        let subnet_id = db.create_subnet(&subnet).await.unwrap();

        // Create static IP assignment
        let static_ip = StaticIP {
            id: None,
            subnet_id,
            mac_address: "AA:BB:CC:DD:EE:FF".to_string(),
            ip_address: Ipv4Addr::new(192, 168, 1, 50),
            hostname: Some("test-host".to_string()),
            enabled: true,
        };
        db.create_static_ip(&static_ip).await.unwrap();

        // Create test packet
        let packet = create_discover_packet("AA:BB:CC:DD:EE:FF");

        // Test handle_discover
        let response = DhcpServer::handle_discover(&packet, &config, &db).await;

        assert!(response.is_some());
        let offer = response.unwrap();
        assert_eq!(offer.yiaddr, Ipv4Addr::new(192, 168, 1, 50));
        assert_eq!(offer.xid, 12345);
        assert_eq!(offer.op, 2); // BOOTREPLY

        // Verify options
        let msg_type = offer.get_message_type();
        assert_eq!(msg_type, Some(MessageType::Offer));
    }

    #[tokio::test]
    async fn test_handle_discover_with_existing_lease() {
        let config = create_test_config();
        let db = InMemoryDatabase::new();

        // Create subnet
        let subnet = create_test_subnet();
        let subnet_id = db.create_subnet(&subnet).await.unwrap();

        // Create an active lease
        let now = chrono::Utc::now().timestamp();
        let lease = Lease {
            id: None,
            subnet_id,
            mac_address: "11:22:33:44:55:66".to_string(),
            ip_address: Ipv4Addr::new(192, 168, 1, 100),
            lease_start: now,
            lease_end: now + 86400,
            hostname: None,
            active: true,
        };
        db.create_lease(&lease).await.unwrap();

        // Create test packet
        let packet = create_discover_packet("11:22:33:44:55:66");

        // Test handle_discover
        let response = DhcpServer::handle_discover(&packet, &config, &db).await;

        assert!(response.is_some());
        let offer = response.unwrap();
        assert_eq!(offer.yiaddr, Ipv4Addr::new(192, 168, 1, 100));
        assert_eq!(offer.xid, 12345);

        let msg_type = offer.get_message_type();
        assert_eq!(msg_type, Some(MessageType::Offer));
    }

    #[tokio::test]
    async fn test_handle_discover_no_allocation() {
        let config = create_test_config();
        let db = InMemoryDatabase::new();

        // Create subnet but no static IP or lease
        let subnet = create_test_subnet();
        db.create_subnet(&subnet).await.unwrap();

        // Create test packet
        let packet = create_discover_packet("99:88:77:66:55:44");

        // Test handle_discover
        let response = DhcpServer::handle_discover(&packet, &config, &db).await;

        // Should return None: no static IP, no lease, and no dynamic ranges
        assert!(response.is_none());
    }

    #[tokio::test]
    async fn test_handle_discover_static_ip_takes_precedence() {
        let config = create_test_config();
        let db = InMemoryDatabase::new();

        // Create subnet
        let subnet = create_test_subnet();
        let subnet_id = db.create_subnet(&subnet).await.unwrap();

        // Create static IP assignment
        let static_ip = StaticIP {
            id: None,
            subnet_id,
            mac_address: "AA:BB:CC:DD:EE:00".to_string(),
            ip_address: Ipv4Addr::new(192, 168, 1, 50),
            hostname: Some("static-host".to_string()),
            enabled: true,
        };
        db.create_static_ip(&static_ip).await.unwrap();

        // Also create a lease for the same MAC
        let now = chrono::Utc::now().timestamp();
        let lease = Lease {
            id: None,
            subnet_id,
            mac_address: "AA:BB:CC:DD:EE:00".to_string(),
            ip_address: Ipv4Addr::new(192, 168, 1, 100),
            lease_start: now,
            lease_end: now + 86400,
            hostname: None,
            active: true,
        };
        db.create_lease(&lease).await.unwrap();

        // Create test packet
        let packet = create_discover_packet("AA:BB:CC:DD:EE:00");

        // Test handle_discover - static IP should take precedence
        let response = DhcpServer::handle_discover(&packet, &config, &db).await;

        assert!(response.is_some());
        let offer = response.unwrap();
        // Should offer the static IP, not the leased IP
        assert_eq!(offer.yiaddr, Ipv4Addr::new(192, 168, 1, 50));
    }

    #[tokio::test]
    async fn test_handle_discover_dynamic_allocation() {
        let config = create_test_config();
        let db = InMemoryDatabase::new();

        let subnet = create_test_subnet();
        let subnet_id = db.create_subnet(&subnet).await.unwrap();

        // Create a dynamic range
        let range = crate::models::DynamicRange {
            id: None,
            subnet_id,
            range_start: Ipv4Addr::new(192, 168, 1, 100),
            range_end: Ipv4Addr::new(192, 168, 1, 200),
            enabled: true,
        };
        db.create_range(&range).await.unwrap();

        let packet = create_discover_packet("AA:BB:CC:DD:EE:11");
        let response = DhcpServer::handle_discover(&packet, &config, &db).await;

        assert!(response.is_some());
        let offer = response.unwrap();
        // Should offer the first IP in the range
        assert_eq!(offer.yiaddr, Ipv4Addr::new(192, 168, 1, 100));
        assert_eq!(offer.get_message_type(), Some(MessageType::Offer));
    }

    #[tokio::test]
    async fn test_handle_discover_dynamic_skips_leased_ips() {
        let config = create_test_config();
        let db = InMemoryDatabase::new();

        let subnet = create_test_subnet();
        let subnet_id = db.create_subnet(&subnet).await.unwrap();

        let range = crate::models::DynamicRange {
            id: None,
            subnet_id,
            range_start: Ipv4Addr::new(192, 168, 1, 100),
            range_end: Ipv4Addr::new(192, 168, 1, 200),
            enabled: true,
        };
        db.create_range(&range).await.unwrap();

        // Lease .100 to another client
        let now = chrono::Utc::now().timestamp();
        let lease = Lease {
            id: None,
            subnet_id,
            mac_address: "11:22:33:44:55:66".to_string(),
            ip_address: Ipv4Addr::new(192, 168, 1, 100),
            lease_start: now,
            lease_end: now + 86400,
            hostname: None,
            active: true,
        };
        db.create_lease(&lease).await.unwrap();

        let packet = create_discover_packet("AA:BB:CC:DD:EE:22");
        let response = DhcpServer::handle_discover(&packet, &config, &db).await;

        assert!(response.is_some());
        // Should skip .100 (leased) and offer .101
        assert_eq!(response.unwrap().yiaddr, Ipv4Addr::new(192, 168, 1, 101));
    }

    #[tokio::test]
    async fn test_handle_request_dynamic_creates_lease() {
        let config = create_test_config();
        let db = InMemoryDatabase::new();

        let subnet = create_test_subnet();
        let subnet_id = db.create_subnet(&subnet).await.unwrap();

        let range = crate::models::DynamicRange {
            id: None,
            subnet_id,
            range_start: Ipv4Addr::new(192, 168, 1, 100),
            range_end: Ipv4Addr::new(192, 168, 1, 200),
            enabled: true,
        };
        db.create_range(&range).await.unwrap();

        let requested = Ipv4Addr::new(192, 168, 1, 100);
        let packet = create_request_packet("AA:BB:CC:DD:EE:33", requested);
        let response = DhcpServer::handle_request(&packet, &config, &db).await;

        assert!(response.is_some());
        let ack = response.unwrap();
        assert_eq!(ack.yiaddr, requested);
        assert_eq!(ack.get_message_type(), Some(MessageType::Ack));

        // Verify lease was persisted
        let lease = db.get_active_lease("AA:BB:CC:DD:EE:33").await.unwrap();
        assert!(lease.is_some());
        assert_eq!(lease.unwrap().ip_address, requested);
    }

    #[tokio::test]
    async fn test_handle_request_dynamic_rejects_stolen_ip() {
        let config = create_test_config();
        let db = InMemoryDatabase::new();

        let subnet = create_test_subnet();
        let subnet_id = db.create_subnet(&subnet).await.unwrap();

        let range = crate::models::DynamicRange {
            id: None,
            subnet_id,
            range_start: Ipv4Addr::new(192, 168, 1, 100),
            range_end: Ipv4Addr::new(192, 168, 1, 200),
            enabled: true,
        };
        db.create_range(&range).await.unwrap();

        // Another client already owns .100
        let now = chrono::Utc::now().timestamp();
        let existing = Lease {
            id: None,
            subnet_id,
            mac_address: "11:22:33:44:55:66".to_string(),
            ip_address: Ipv4Addr::new(192, 168, 1, 100),
            lease_start: now,
            lease_end: now + 86400,
            hostname: None,
            active: true,
        };
        db.create_lease(&existing).await.unwrap();

        let packet = create_request_packet("AA:BB:CC:DD:EE:44", Ipv4Addr::new(192, 168, 1, 100));
        let response = DhcpServer::handle_request(&packet, &config, &db).await;

        // Should be rejected
        assert!(response.is_none());
    }

    #[tokio::test]
    async fn test_handle_request_with_static_ip() {
        let config = create_test_config();
        let db = InMemoryDatabase::new();

        // Create subnet
        let subnet = create_test_subnet();
        let subnet_id = db.create_subnet(&subnet).await.unwrap();

        // Create static IP assignment
        let static_ip = StaticIP {
            id: None,
            subnet_id,
            mac_address: "AA:BB:CC:DD:EE:FF".to_string(),
            ip_address: Ipv4Addr::new(192, 168, 1, 50),
            hostname: Some("test-host".to_string()),
            enabled: true,
        };
        db.create_static_ip(&static_ip).await.unwrap();

        // Create request packet
        let packet = create_request_packet("AA:BB:CC:DD:EE:FF", Ipv4Addr::new(192, 168, 1, 50));

        // Test handle_request
        let response = DhcpServer::handle_request(&packet, &config, &db).await;

        assert!(response.is_some());
        let ack = response.unwrap();
        assert_eq!(ack.yiaddr, Ipv4Addr::new(192, 168, 1, 50));
        assert_eq!(ack.xid, 67890);
        assert_eq!(ack.op, 2); // BOOTREPLY

        // Verify it's an ACK
        let msg_type = ack.get_message_type();
        assert_eq!(msg_type, Some(MessageType::Ack));
    }

    #[tokio::test]
    async fn test_handle_request_with_wrong_static_ip() {
        let config = create_test_config();
        let db = InMemoryDatabase::new();

        // Create subnet
        let subnet = create_test_subnet();
        let subnet_id = db.create_subnet(&subnet).await.unwrap();

        // Create static IP assignment
        let static_ip = StaticIP {
            id: None,
            subnet_id,
            mac_address: "AA:BB:CC:DD:EE:FF".to_string(),
            ip_address: Ipv4Addr::new(192, 168, 1, 50),
            hostname: Some("test-host".to_string()),
            enabled: true,
        };
        db.create_static_ip(&static_ip).await.unwrap();

        // Request a different IP than the static one
        let packet = create_request_packet("AA:BB:CC:DD:EE:FF", Ipv4Addr::new(192, 168, 1, 100));

        // Test handle_request - should return None as requested IP doesn't match static IP
        let response = DhcpServer::handle_request(&packet, &config, &db).await;

        assert!(response.is_none());
    }

    #[tokio::test]
    async fn test_handle_request_without_requested_ip() {
        use dhcp_proto::MacAddress;

        let config = create_test_config();
        let db = InMemoryDatabase::new();

        // Create subnet
        let subnet = create_test_subnet();
        db.create_subnet(&subnet).await.unwrap();

        // Create request packet without RequestedIpAddress option
        let mut packet = DhcpPacket::new();
        packet.op = 1;
        packet.xid = 67890;
        packet.chaddr = MacAddress::from_string("AA:BB:CC:DD:EE:FF").unwrap();
        packet
            .options
            .push(DhcpOption::MessageType(MessageType::Request));

        // Test handle_request - should return None without requested IP
        let response = DhcpServer::handle_request(&packet, &config, &db).await;

        assert!(response.is_none());
    }

    #[tokio::test]
    async fn test_handle_release_with_active_lease() {
        let db = InMemoryDatabase::new();

        // Create subnet
        let subnet = create_test_subnet();
        let subnet_id = db.create_subnet(&subnet).await.unwrap();

        // Create an active lease
        let now = chrono::Utc::now().timestamp();
        let lease = Lease {
            id: None,
            subnet_id,
            mac_address: "11:22:33:44:55:66".to_string(),
            ip_address: Ipv4Addr::new(192, 168, 1, 100),
            lease_start: now,
            lease_end: now + 86400,
            hostname: None,
            active: true,
        };
        let _lease_id = db.create_lease(&lease).await.unwrap();

        // Verify lease exists and is active
        let active_lease = db.get_active_lease("11:22:33:44:55:66").await.unwrap();
        assert!(active_lease.is_some());

        // Create release packet
        let packet = create_release_packet("11:22:33:44:55:66");

        // Test handle_release
        DhcpServer::handle_release(&packet, &db).await;

        // Verify lease has been expired
        let active_lease_after = db.get_active_lease("11:22:33:44:55:66").await.unwrap();
        assert!(active_lease_after.is_none());
    }

    #[tokio::test]
    async fn test_handle_release_without_lease() {
        let db = InMemoryDatabase::new();

        // Create subnet (for completeness)
        let subnet = create_test_subnet();
        db.create_subnet(&subnet).await.unwrap();

        // Create release packet for a MAC that has no lease
        let packet = create_release_packet("99:88:77:66:55:44");

        // Test handle_release - should not fail even without lease
        DhcpServer::handle_release(&packet, &db).await;

        // No assertion needed - just verify it doesn't panic
    }
}
