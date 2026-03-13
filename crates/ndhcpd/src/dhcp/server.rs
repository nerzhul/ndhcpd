use std::collections::HashSet;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::os::unix::io::AsRawFd;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use super::packet::{DhcpOption, DhcpPacket, MessageType};
use crate::config::Config;
use crate::db::{Database, DynDatabase};

const DHCP_SERVER_PORT: u16 = 67;
const DHCP_CLIENT_PORT: u16 = 68;

/// Enable reception of per-packet interface information through ancillary data.
///
/// * Linux   – `IP_PKTINFO` delivers `struct in_pktinfo` with `ipi_ifindex`.
/// * FreeBSD – `IP_RECVIF` delivers `struct sockaddr_dl` with `sdl_index`.
///
/// After this call every `recvmsg(2)` on the socket will include a `IPPROTO_IP`
/// control message that identifies the interface the datagram arrived on.
fn enable_recv_interface(socket: &UdpSocket) -> anyhow::Result<()> {
    #[cfg(target_os = "linux")]
    let optname = libc::IP_PKTINFO;
    #[cfg(target_os = "freebsd")]
    let optname = libc::IP_RECVIF;
    #[cfg(not(any(target_os = "linux", target_os = "freebsd")))]
    return Err(anyhow::anyhow!(
        "enable_recv_interface: unsupported platform"
    ));

    let one: libc::c_int = 1;
    let ret = unsafe {
        libc::setsockopt(
            socket.as_raw_fd(),
            libc::IPPROTO_IP,
            optname,
            &one as *const libc::c_int as *const libc::c_void,
            std::mem::size_of::<libc::c_int>() as libc::socklen_t,
        )
    };
    if ret != 0 {
        return Err(anyhow::anyhow!(
            "setsockopt IP_RECVIF/IP_PKTINFO: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}

/// Receive one UDP datagram and return *(bytes_received, source_addr, interface_name)*.
///
/// Uses `recvmsg(2)` to harvest ancillary data that identifies the network
/// interface the datagram arrived on (`IP_RECVIF` on FreeBSD, `IP_PKTINFO` on
/// Linux).  If the interface cannot be determined, `"unknown"` is returned as
/// the interface name so that the caller can apply its filter and drop the packet.
fn recv_with_interface(
    socket: &UdpSocket,
    buf: &mut [u8],
) -> anyhow::Result<(usize, SocketAddr, String)> {
    // 256 bytes is more than enough for one IP_RECVIF / IP_PKTINFO cmsg.
    let mut ctrl = [0u8; 256];
    let mut src_storage: libc::sockaddr_storage = unsafe { std::mem::zeroed() };

    let (len, iface_name) = unsafe {
        let mut iov = libc::iovec {
            iov_base: buf.as_mut_ptr() as *mut libc::c_void,
            iov_len: buf.len(),
        };
        let mut msg: libc::msghdr = std::mem::zeroed();
        msg.msg_name = &mut src_storage as *mut _ as *mut libc::c_void;
        msg.msg_namelen =
            std::mem::size_of::<libc::sockaddr_storage>() as libc::socklen_t;
        msg.msg_iov = &mut iov;
        msg.msg_iovlen = 1 as _; // c_int on FreeBSD, size_t on Linux
        msg.msg_control = ctrl.as_mut_ptr() as *mut libc::c_void;
        msg.msg_controllen = ctrl.len() as _; // socklen_t on FreeBSD, size_t on Linux

        let n = libc::recvmsg(socket.as_raw_fd(), &mut msg, 0);
        if n < 0 {
            return Err(std::io::Error::last_os_error().into());
        }

        let name = parse_interface_from_cmsg(&msg)
            .unwrap_or_else(|| "unknown".to_string());
        (n as usize, name)
    };

    let src = unsafe { sockaddr_storage_to_socketaddr(&src_storage) }
        .unwrap_or_else(|| SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 0));

    Ok((len, src, iface_name))
}

/// Extract the incoming interface name from `recvmsg` ancillary data.
///
/// FreeBSD: `IPPROTO_IP` / `IP_RECVIF` → `sockaddr_dl.sdl_index`
/// Linux:   `IPPROTO_IP` / `IP_PKTINFO` → `in_pktinfo.ipi_ifindex`
#[cfg(target_os = "freebsd")]
unsafe fn parse_interface_from_cmsg(msg: &libc::msghdr) -> Option<String> {
    let mut cmsg = libc::CMSG_FIRSTHDR(msg);
    while !cmsg.is_null() {
        let cm = &*cmsg;
        if cm.cmsg_level == libc::IPPROTO_IP && cm.cmsg_type == libc::IP_RECVIF {
            let sdl = libc::CMSG_DATA(cmsg) as *const libc::sockaddr_dl;
            let idx = (*sdl).sdl_index as libc::c_uint;
            let mut name_buf = [0u8; libc::IFNAMSIZ];
            if !libc::if_indextoname(
                idx,
                name_buf.as_mut_ptr() as *mut libc::c_char,
            )
            .is_null()
            {
                return Some(
                    std::ffi::CStr::from_ptr(name_buf.as_ptr() as *const libc::c_char)
                        .to_string_lossy()
                        .into_owned(),
                );
            }
        }
        cmsg = libc::CMSG_NXTHDR(msg, cmsg);
    }
    None
}

#[cfg(target_os = "linux")]
unsafe fn parse_interface_from_cmsg(msg: &libc::msghdr) -> Option<String> {
    let mut cmsg = libc::CMSG_FIRSTHDR(msg);
    while !cmsg.is_null() {
        let cm = &*cmsg;
        if cm.cmsg_level == libc::IPPROTO_IP && cm.cmsg_type == libc::IP_PKTINFO {
            let pktinfo = libc::CMSG_DATA(cmsg) as *const libc::in_pktinfo;
            let idx = (*pktinfo).ipi_ifindex as libc::c_uint;
            let mut name_buf = [0u8; libc::IFNAMSIZ];
            if !libc::if_indextoname(
                idx,
                name_buf.as_mut_ptr() as *mut libc::c_char,
            )
            .is_null()
            {
                return Some(
                    std::ffi::CStr::from_ptr(name_buf.as_ptr() as *const libc::c_char)
                        .to_string_lossy()
                        .into_owned(),
                );
            }
        }
        cmsg = libc::CMSG_NXTHDR(msg, cmsg);
    }
    None
}

#[cfg(not(any(target_os = "freebsd", target_os = "linux")))]
unsafe fn parse_interface_from_cmsg(_msg: &libc::msghdr) -> Option<String> {
    None
}

/// Convert a `sockaddr_storage` populated by `recvmsg` into a `SocketAddr`.
unsafe fn sockaddr_storage_to_socketaddr(
    ss: &libc::sockaddr_storage,
) -> Option<SocketAddr> {
    if ss.ss_family as i32 == libc::AF_INET {
        let sin = ss as *const _ as *const libc::sockaddr_in;
        let ip = Ipv4Addr::from(u32::from_be((*sin).sin_addr.s_addr));
        let port = u16::from_be((*sin).sin_port);
        Some(SocketAddr::new(ip.into(), port))
    } else {
        None
    }
}

/// Creates a UDP socket used exclusively for sending DHCP broadcast responses.
///
/// On FreeBSD, broadcasting to `255.255.255.255` from a socket bound to
/// `0.0.0.0` fails with `ENETUNREACH`/`EHOSTUNREACH` because the kernel
/// consults the routing table and finds no entry for the limited broadcast.
///
/// The fix combines two things:
/// 1. Bind the send socket to the interface's own IPv4 address so the kernel
///    knows which interface the packet should leave on.
/// 2. Set `SO_DONTROUTE` so the kernel bypasses the routing table entirely
///    and sends directly on the locally-connected network.
///
/// This socket is only used when the DHCP BROADCAST flag is set in the client
/// request (RFC 2131 §4.1).  Most modern clients do NOT set this flag, so the
/// common path unicasts to `yiaddr` using the regular socket instead.
///
/// On Linux, `SO_BINDTODEVICE` on the receive socket already pins the
/// interface, and the routing table normally has a broadcast route, so
/// neither workaround is needed.
fn create_broadcast_send_socket(interface: &str) -> anyhow::Result<UdpSocket> {
    #[cfg(target_os = "freebsd")]
    {
        let iface_ips = get_interface_ips(interface);
        if let Some(iface_ip) = iface_ips.first() {
            let send_addr = SocketAddr::new((*iface_ip).into(), 0);
            let s = UdpSocket::bind(send_addr)?;
            s.set_broadcast(true)?;
            // Bypass the routing table so that 255.255.255.255 is sent
            // directly on the interface's connected link.
            let one: libc::c_int = 1;
            let ret = unsafe {
                libc::setsockopt(
                    s.as_raw_fd(),
                    libc::SOL_SOCKET,
                    libc::SO_DONTROUTE,
                    &one as *const libc::c_int as *const libc::c_void,
                    std::mem::size_of::<libc::c_int>() as libc::socklen_t,
                )
            };
            if ret != 0 {
                return Err(anyhow::anyhow!(
                    "Failed to set SO_DONTROUTE on broadcast send socket: {}",
                    std::io::Error::last_os_error()
                ));
            }
            return Ok(s);
        }
        warn!(
            "No IPv4 address found on interface {}; falling back to 0.0.0.0 for broadcast sends",
            interface
        );
    }

    let s = UdpSocket::bind(SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 0))?;
    s.set_broadcast(true)?;
    Ok(s)
}

/// Determines the UDP destination for a DHCP server response per RFC 2131 §4.1.
///
/// Rules applied in order:
/// 1. `giaddr` != 0 (relay agent): send to relay agent on port 67.
/// 2. `ciaddr` != 0 (client has a configured IP): unicast to `ciaddr:68`.
/// 3. BROADCAST flag set in client request: broadcast to `255.255.255.255:68`.
/// 4. Otherwise: unicast to the offered/assigned IP (`yiaddr`) on port 68.
fn response_dest(
    request: &DhcpPacket,
    response: &DhcpPacket,
) -> SocketAddr {
    const BROADCAST_FLAG: u16 = 0x8000;
    let unspecified = Ipv4Addr::UNSPECIFIED;

    if request.giaddr != unspecified {
        // Relay agent present – return to relay on the DHCP server port
        SocketAddr::new(request.giaddr.into(), DHCP_SERVER_PORT)
    } else if request.ciaddr != unspecified {
        // Client already has an IP address (RENEWING/REBINDING)
        SocketAddr::new(request.ciaddr.into(), DHCP_CLIENT_PORT)
    } else if request.flags & BROADCAST_FLAG != 0 {
        // Client explicitly requested a broadcast reply
        SocketAddr::new(Ipv4Addr::new(255, 255, 255, 255).into(), DHCP_CLIENT_PORT)
    } else {
        // Unicast to the offered/assigned address – works on all platforms
        // without any special socket options.
        SocketAddr::new(response.yiaddr.into(), DHCP_CLIENT_PORT)
    }
}

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

        // A single socket listens on 0.0.0.0:67. The incoming interface is
        // identified from ancillary data (IP_RECVIF / IP_PKTINFO) and packets
        // are filtered against config.listen_interfaces before processing.
        let server = Arc::new(Self {
            config: Arc::clone(&self.config),
            db: Arc::clone(&self.db),
        });

        tokio::spawn(async move {
            if let Err(e) = server.listen_loop().await {
                error!("DHCP listener error: {}", e);
            }
        });

        // Spawn a background task that periodically purges expired leases.
        let cleanup_db = Arc::clone(&self.db);
        let cleanup_interval = self.config.dhcp.lease_cleanup_interval_minutes;
        tokio::spawn(async move {
            let interval =
                tokio::time::Duration::from_secs(cleanup_interval as u64 * 60);
            loop {
                tokio::time::sleep(interval).await;
                match cleanup_db.delete_expired_leases().await {
                    Ok(n) if n > 0 => info!("Lease cleanup: deleted {} expired lease(s)", n),
                    Ok(_) => debug!("Lease cleanup: no expired leases"),
                    Err(e) => warn!("Lease cleanup failed: {}", e),
                }
            }
        });

        // Keep running
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
        }
    }

    async fn listen_loop(&self) -> anyhow::Result<()> {
        let addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), DHCP_SERVER_PORT);
        let socket = UdpSocket::bind(addr)?;
        socket.set_broadcast(true)?;

        // Enable ancillary-data delivery so recvmsg can report which interface
        // each datagram arrived on (IP_RECVIF on FreeBSD, IP_PKTINFO on Linux).
        enable_recv_interface(&socket)?;

        // Per-interface broadcast send sockets, created lazily on first use.
        // Required on FreeBSD when the BROADCAST flag is set by the client
        // (see create_broadcast_send_socket for the rationale).
        let mut bcast_sockets: std::collections::HashMap<String, Option<UdpSocket>> =
            std::collections::HashMap::new();

        info!(
            "DHCP server listening on 0.0.0.0:{} (managed interfaces: {})",
            DHCP_SERVER_PORT,
            self.config.listen_interfaces.join(", ")
        );

        let mut buf = vec![0u8; 1500];

        loop {
            let (len, src, iface_name) = match recv_with_interface(&socket, &mut buf) {
                Ok(v) => v,
                Err(e) => {
                    warn!("recvmsg error: {}", e);
                    continue;
                }
            };

            // Drop packets arriving on interfaces we do not manage.
            if !self.config.listen_interfaces.iter().any(|i| i == &iface_name) {
                debug!(
                    "Ignoring DHCP packet on unmanaged interface {}",
                    iface_name
                );
                continue;
            }

            debug!("Received {} bytes from {} on {}", len, src, iface_name);

            let packet_data = &buf[..len];
            let packet = match DhcpPacket::parse(packet_data) {
                Ok(p) => p,
                Err(e) => {
                    warn!("Failed to parse DHCP packet: {}", e);
                    continue;
                }
            };

            let iface_ips = get_interface_ips(&iface_name);
            let response = Self::handle_packet(&packet, &iface_ips, &self.config, &*self.db).await;

            if let Some(response_packet) = response {
                let response_bytes = response_packet.to_bytes();
                // Determine destination per RFC 2131 §4.1:
                //   giaddr != 0        → relay agent on port 67
                //   ciaddr != 0        → unicast to ciaddr:68
                //   BROADCAST flag set → 255.255.255.255:68 (broadcast socket)
                //   otherwise          → unicast to yiaddr:68
                let dest = response_dest(&packet, &response_packet);
                let is_broadcast =
                    dest.ip() == std::net::IpAddr::V4(Ipv4Addr::new(255, 255, 255, 255));

                let result = if is_broadcast {
                    // Use a per-interface socket with SO_DONTROUTE for FreeBSD
                    // compatibility (see create_broadcast_send_socket).
                    let bcast_sock = bcast_sockets
                        .entry(iface_name.clone())
                        .or_insert_with(|| create_broadcast_send_socket(&iface_name).ok());
                    match bcast_sock {
                        Some(s) => s.send_to(&response_bytes, dest),
                        None => socket.send_to(&response_bytes, dest),
                    }
                } else {
                    socket.send_to(&response_bytes, dest)
                };

                if let Err(e) = result {
                    warn!("Failed to send DHCP response to {}: {}", dest, e);
                }
            }
        }
    }

    async fn handle_packet(
        packet: &DhcpPacket,
        iface_ips: &[Ipv4Addr],
        config: &Config,
        db: &dyn Database,
    ) -> Option<DhcpPacket> {
        let msg_type = packet.get_message_type()?;
        let mac = packet.chaddr.to_string();

        match msg_type {
            MessageType::Discover => {
                info!("DHCP DISCOVER from {}", mac);
                Self::handle_discover(packet, iface_ips, config, db).await
            }
            MessageType::Request => {
                info!("DHCP REQUEST from {}", mac);
                Self::handle_request(packet, iface_ips, config, db).await
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
        iface_ips: &[Ipv4Addr],
        config: &Config,
        db: &dyn Database,
    ) -> Option<DhcpPacket> {
        let mac = packet.chaddr.to_string();

        // Check for static IP assignment on a subnet reachable via this interface
        if let Ok(Some(static_ip)) = db.get_static_ip_by_mac(&mac).await {
            let subnet = db.get_subnet(static_ip.subnet_id).await.ok()??;
            if iface_in_subnet(iface_ips, &subnet) {
                return Some(Self::create_offer(
                    packet,
                    static_ip.ip_address,
                    &subnet,
                    config,
                ));
            }
        }

        // Check for an existing lease on a subnet reachable via this interface
        if let Ok(Some(lease)) = db.get_active_lease(&mac).await {
            let subnet = db.get_subnet(lease.subnet_id).await.ok()??;
            if iface_in_subnet(iface_ips, &subnet) {
                return Some(Self::create_offer(
                    packet,
                    lease.ip_address,
                    &subnet,
                    config,
                ));
            }
        }

        // Allocate a new IP from an enabled dynamic range, scoped to subnets
        // reachable via this interface.
        let subnets = match db.list_active_subnets().await {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to list subnets: {}", e);
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

        for subnet in subnets.iter().filter(|s| iface_in_subnet(iface_ips, s)) {
            let subnet_id = match subnet.id {
                Some(id) => id,
                None => continue,
            };
            let ranges = match db.list_ranges(Some(subnet_id)).await {
                Ok(r) => r,
                Err(e) => {
                    error!("Failed to list ranges for subnet {}: {}", subnet_id, e);
                    continue;
                }
            };

            for range in ranges.iter().filter(|r| r.enabled) {
                let start = u32::from(range.range_start);
                let end = u32::from(range.range_end);

                for ip_u32 in start..=end {
                    let candidate = Ipv4Addr::from(ip_u32);
                    if !leased_ips.contains(&candidate) {
                        debug!("Offering dynamic IP {} to {}", candidate, mac);
                        return Some(Self::create_offer(packet, candidate, subnet, config));
                    }
                }
            }
        }

        warn!("No free IP available for DISCOVER from {}", mac);
        None
    }

    async fn handle_request(
        packet: &DhcpPacket,
        iface_ips: &[Ipv4Addr],
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
                if !iface_in_subnet(iface_ips, &subnet) {
                    warn!(
                        "Client {} static IP {} belongs to a subnet not reachable via this interface",
                        mac, requested_ip
                    );
                    return None;
                }
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

        // Find a range that covers the requested IP and belongs to a subnet matching this interface
        let mut matching_range_and_subnet = None;
        for r in ranges.iter() {
            if !r.enabled {
                continue;
            }
            if u32::from(r.range_start) <= u32::from(requested_ip)
                && u32::from(requested_ip) <= u32::from(r.range_end)
            {
                let subnet = match db.get_subnet(r.subnet_id).await {
                    Ok(Some(s)) => s,
                    _ => continue,
                };
                if iface_in_subnet(iface_ips, &subnet) {
                    matching_range_and_subnet = Some((r.clone(), subnet));
                    break;
                }
            }
        }
        let (matching_range, subnet) = matching_range_and_subnet?;

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

/// Returns the IPv4 addresses assigned to the given network interface.
fn get_interface_ips(interface: &str) -> Vec<Ipv4Addr> {
    let mut ips = Vec::new();
    unsafe {
        let mut ifaddrs: *mut libc::ifaddrs = std::ptr::null_mut();
        if libc::getifaddrs(&mut ifaddrs) != 0 {
            return ips;
        }
        let mut cur = ifaddrs;
        while !cur.is_null() {
            let ifa = &*cur;
            if !ifa.ifa_addr.is_null() {
                let name = std::ffi::CStr::from_ptr(ifa.ifa_name).to_string_lossy();
                if name == interface && (*ifa.ifa_addr).sa_family as i32 == libc::AF_INET {
                    let sin = ifa.ifa_addr as *const libc::sockaddr_in;
                    let ip = u32::from_be((*sin).sin_addr.s_addr);
                    ips.push(Ipv4Addr::from(ip));
                }
            }
            cur = ifa.ifa_next;
        }
        libc::freeifaddrs(ifaddrs);
    }
    ips
}

/// Returns true if any IP on the interface belongs to the given subnet.
fn iface_in_subnet(iface_ips: &[Ipv4Addr], subnet: &crate::models::Subnet) -> bool {
    let mask: u32 = if subnet.netmask == 0 {
        0
    } else {
        !0u32 << (32 - subnet.netmask)
    };
    let net = u32::from(subnet.network) & mask;
    iface_ips.iter().any(|ip| u32::from(*ip) & mask == net)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::InMemoryDatabase;
    use crate::dhcp::test_helpers::*;
    use crate::models::{Lease, StaticIP};

    fn make_subnet(network: Ipv4Addr, netmask: u8) -> crate::models::Subnet {
        crate::models::Subnet {
            id: None,
            network,
            netmask,
            gateway: network,
            dns_servers: vec![],
            domain_name: None,
            enabled: true,
        }
    }

    #[test]
    fn test_iface_in_subnet_match() {
        let subnet = make_subnet(Ipv4Addr::new(192, 168, 1, 0), 24);
        assert!(iface_in_subnet(&[Ipv4Addr::new(192, 168, 1, 1)], &subnet));
    }

    #[test]
    fn test_iface_in_subnet_last_address() {
        let subnet = make_subnet(Ipv4Addr::new(192, 168, 1, 0), 24);
        assert!(iface_in_subnet(&[Ipv4Addr::new(192, 168, 1, 254)], &subnet));
    }

    #[test]
    fn test_iface_in_subnet_no_match() {
        let subnet = make_subnet(Ipv4Addr::new(192, 168, 1, 0), 24);
        assert!(!iface_in_subnet(&[Ipv4Addr::new(10, 0, 0, 1)], &subnet));
    }

    #[test]
    fn test_iface_in_subnet_empty_ips() {
        let subnet = make_subnet(Ipv4Addr::new(192, 168, 1, 0), 24);
        assert!(!iface_in_subnet(&[], &subnet));
    }

    #[test]
    fn test_iface_in_subnet_multiple_ips_one_matches() {
        let subnet = make_subnet(Ipv4Addr::new(10, 0, 0, 0), 8);
        let ips = [Ipv4Addr::new(192, 168, 1, 1), Ipv4Addr::new(10, 5, 6, 7)];
        assert!(iface_in_subnet(&ips, &subnet));
    }

    #[test]
    fn test_iface_in_subnet_slash16() {
        let subnet = make_subnet(Ipv4Addr::new(172, 16, 0, 0), 16);
        assert!(iface_in_subnet(&[Ipv4Addr::new(172, 16, 42, 1)], &subnet));
        assert!(!iface_in_subnet(&[Ipv4Addr::new(172, 17, 0, 1)], &subnet));
    }

    #[test]
    fn test_iface_in_subnet_slash32() {
        let subnet = make_subnet(Ipv4Addr::new(10, 0, 0, 1), 32);
        assert!(iface_in_subnet(&[Ipv4Addr::new(10, 0, 0, 1)], &subnet));
        assert!(!iface_in_subnet(&[Ipv4Addr::new(10, 0, 0, 2)], &subnet));
    }

    #[test]
    fn test_iface_in_subnet_slash0() {
        // /0 matches everything
        let subnet = make_subnet(Ipv4Addr::new(0, 0, 0, 0), 0);
        assert!(iface_in_subnet(&[Ipv4Addr::new(1, 2, 3, 4)], &subnet));
        assert!(iface_in_subnet(
            &[Ipv4Addr::new(255, 255, 255, 255)],
            &subnet
        ));
    }

    #[tokio::test]
    async fn test_handle_discover_with_static_ip() {
        let config = create_test_config();
        let db = InMemoryDatabase::new();

        // Create subnet
        let subnet = create_test_subnet();
        let subnet_id = db.create_subnet(&subnet).await.unwrap();

        // Create static IP assignment
        let static_ip = StaticIP {
            subnet_id,
            mac_address: "AA:BB:CC:DD:EE:FF".to_string(),
            ip_address: Ipv4Addr::new(192, 168, 1, 50),
            hostname: Some("test-host".to_string()),
        };
        db.create_static_ip(&static_ip).await.unwrap();

        // Create test packet
        let packet = create_discover_packet("AA:BB:CC:DD:EE:FF");

        // Test handle_discover
        let response =
            DhcpServer::handle_discover(&packet, &[Ipv4Addr::new(192, 168, 1, 1)], &config, &db)
                .await;

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
        };
        db.create_lease(&lease).await.unwrap();

        // Create test packet
        let packet = create_discover_packet("11:22:33:44:55:66");

        // Test handle_discover
        let response =
            DhcpServer::handle_discover(&packet, &[Ipv4Addr::new(192, 168, 1, 1)], &config, &db)
                .await;

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
        let response =
            DhcpServer::handle_discover(&packet, &[Ipv4Addr::new(192, 168, 1, 1)], &config, &db)
                .await;

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
            subnet_id,
            mac_address: "AA:BB:CC:DD:EE:00".to_string(),
            ip_address: Ipv4Addr::new(192, 168, 1, 50),
            hostname: Some("static-host".to_string()),
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
        };
        db.create_lease(&lease).await.unwrap();

        // Create test packet
        let packet = create_discover_packet("AA:BB:CC:DD:EE:00");

        // Test handle_discover - static IP should take precedence
        let response =
            DhcpServer::handle_discover(&packet, &[Ipv4Addr::new(192, 168, 1, 1)], &config, &db)
                .await;

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
        let response =
            DhcpServer::handle_discover(&packet, &[Ipv4Addr::new(192, 168, 1, 1)], &config, &db)
                .await;

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
        };
        db.create_lease(&lease).await.unwrap();

        let packet = create_discover_packet("AA:BB:CC:DD:EE:22");
        let response =
            DhcpServer::handle_discover(&packet, &[Ipv4Addr::new(192, 168, 1, 1)], &config, &db)
                .await;

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
        let response =
            DhcpServer::handle_request(&packet, &[Ipv4Addr::new(192, 168, 1, 1)], &config, &db)
                .await;

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
        };
        db.create_lease(&existing).await.unwrap();

        let packet = create_request_packet("AA:BB:CC:DD:EE:44", Ipv4Addr::new(192, 168, 1, 100));
        let response =
            DhcpServer::handle_request(&packet, &[Ipv4Addr::new(192, 168, 1, 1)], &config, &db)
                .await;

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
            subnet_id,
            mac_address: "AA:BB:CC:DD:EE:FF".to_string(),
            ip_address: Ipv4Addr::new(192, 168, 1, 50),
            hostname: Some("test-host".to_string()),
        };
        db.create_static_ip(&static_ip).await.unwrap();

        // Create request packet
        let packet = create_request_packet("AA:BB:CC:DD:EE:FF", Ipv4Addr::new(192, 168, 1, 50));

        // Test handle_request
        let response =
            DhcpServer::handle_request(&packet, &[Ipv4Addr::new(192, 168, 1, 1)], &config, &db)
                .await;

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
            subnet_id,
            mac_address: "AA:BB:CC:DD:EE:FF".to_string(),
            ip_address: Ipv4Addr::new(192, 168, 1, 50),
            hostname: Some("test-host".to_string()),
        };
        db.create_static_ip(&static_ip).await.unwrap();

        // Request a different IP than the static one
        let packet = create_request_packet("AA:BB:CC:DD:EE:FF", Ipv4Addr::new(192, 168, 1, 100));

        // Test handle_request - should return None as requested IP doesn't match static IP
        let response =
            DhcpServer::handle_request(&packet, &[Ipv4Addr::new(192, 168, 1, 1)], &config, &db)
                .await;

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
        let response =
            DhcpServer::handle_request(&packet, &[Ipv4Addr::new(192, 168, 1, 1)], &config, &db)
                .await;

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
