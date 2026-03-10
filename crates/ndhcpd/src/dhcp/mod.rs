pub mod packet;
pub mod server;

#[cfg(test)]
pub mod test_helpers;

pub use server::DhcpServer;

// Re-export types from dhcp-proto
pub use dhcp_proto::{DhcpOption, DhcpPacket, MacAddress, MessageType};

/// DHCP message types (for backward compatibility)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DhcpMessageType {
    Discover = 1,
    Offer = 2,
    Request = 3,
    Decline = 4,
    Ack = 5,
    Nak = 6,
    Release = 7,
    Inform = 8,
}

impl DhcpMessageType {
    pub fn from_u8(value: u8) -> Option<Self> {
        MessageType::from_u8(value).map(|mt| match mt {
            MessageType::Discover => Self::Discover,
            MessageType::Offer => Self::Offer,
            MessageType::Request => Self::Request,
            MessageType::Decline => Self::Decline,
            MessageType::Ack => Self::Ack,
            MessageType::Nak => Self::Nak,
            MessageType::Release => Self::Release,
            MessageType::Inform => Self::Inform,
        })
    }
}
