use crate::config::NetworkAddresses;
use pnet::{
    packet::{
        arp::{ArpHardwareTypes, ArpOperations, ArpPacket, MutableArpPacket},
        ethernet::{EtherTypes, EthernetPacket, MutableEthernetPacket},
        ip::IpNextHeaderProtocols,
        ipv4::Ipv4Packet,
        udp::UdpPacket,
        MutablePacket, Packet,
    },
    util::MacAddr,
};
use snafu::ResultExt;
use std::convert::TryInto;
use std::net::Ipv4Addr;

pub enum Event {
    Ignored,
    Connected(MacAddr),
    Alive { mac: MacAddr, ip: Ipv4Addr },
}

macro_rules! try_event {
    ($expr:expr) => {
        match $expr {
            Some(val) => val,
            None => {
                return Event::Ignored;
            }
        }
    };
    ($expr:expr,) => {
        try_event!($expr)
    };
}

pub fn parse_packet(data: &[u8]) -> Event {
    let ethernet = EthernetPacket::new(data).unwrap();
    match ethernet.get_ethertype() {
        EtherTypes::Ipv4 => parse_ipv4_packet(&ethernet),
        EtherTypes::Arp => parse_arp_packet(&ethernet),
        _ => Event::Ignored,
    }
}

fn parse_ipv4_packet(ethernet: &EthernetPacket) -> Event {
    let header = try_event!(Ipv4Packet::new(ethernet.payload()));
    if let IpNextHeaderProtocols::Udp = header.get_next_level_protocol() {
        let udp = try_event!(UdpPacket::new(header.payload()));
        if udp.get_source() == 68 && udp.get_destination() == 67 {
            return Event::Connected(ethernet.get_source());
        }
    }
    Event::Ignored
}

fn parse_arp_packet(ethernet: &EthernetPacket) -> Event {
    let header = try_event!(ArpPacket::new(ethernet.payload()));
    let op = header.get_operation();
    if (op == ArpOperations::Request
        && header.get_sender_proto_addr() == header.get_target_proto_addr())
        || op == ArpOperations::Reply
    {
        return Event::Alive {
            mac: header.get_sender_hw_addr(),
            ip: header.get_sender_proto_addr(),
        };
    }
    Event::Ignored
}

pub struct Socket {
    socket: socket2::Socket,
    address: socket2::SockAddr,
}

impl Socket {
    pub fn new(interface_index: u32) -> crate::Result<Socket> {
        Ok(Socket {
            socket: socket2::Socket::new(
                libc::AF_PACKET.into(),
                socket2::Type::raw(),
                Some(libc::ETH_P_ALL.to_be().into()),
            )
            .with_context(|| crate::error::SendError)?,
            address: unsafe {
                let mut addr: libc::sockaddr_ll = std::mem::zeroed();
                addr.sll_family = libc::AF_PACKET.try_into().unwrap();
                addr.sll_ifindex = interface_index.try_into().unwrap();
                addr.sll_halen = 6;
                addr.sll_protocol = (libc::ETH_P_ARP as u16).to_be();
                socket2::SockAddr::from_raw_parts(
                    (&addr as *const libc::sockaddr_ll) as *const _,
                    std::mem::size_of::<libc::sockaddr_ll>().try_into().unwrap(),
                )
            },
        })
    }

    pub fn send_arp_request(
        &self,
        us: &NetworkAddresses,
        them: &NetworkAddresses,
    ) -> crate::Result<()> {
        let mut buffer = [0u8; 42];
        let mut ethernet = MutableEthernetPacket::new(&mut buffer).unwrap();

        ethernet.set_destination(them.mac);
        ethernet.set_source(us.mac);
        ethernet.set_ethertype(EtherTypes::Arp);

        let payload_buffer = &mut ethernet.payload_mut();
        let mut arp = MutableArpPacket::new(payload_buffer).unwrap();
        arp.set_hardware_type(ArpHardwareTypes::Ethernet);
        arp.set_protocol_type(EtherTypes::Ipv4);
        arp.set_hw_addr_len(6);
        arp.set_proto_addr_len(4);
        arp.set_operation(ArpOperations::Request);
        arp.set_sender_hw_addr(us.mac);
        arp.set_sender_proto_addr(us.ip);
        arp.set_target_hw_addr(them.mac);
        arp.set_target_proto_addr(them.ip);

        self.socket
            .send_to(ethernet.packet(), &self.address)
            .with_context(|| crate::error::SendError)?;

        Ok(())
    }
}
