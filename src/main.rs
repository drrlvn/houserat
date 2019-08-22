use mac_address::MacAddress;
use std::convert::TryInto;
use std::path::PathBuf;
use structopt::StructOpt;

mod config;
mod error;
mod mac_address;
mod telegram;

#[derive(Debug, structopt::StructOpt)]
#[structopt(rename_all = "kebab-case")]
struct Opt {
    #[structopt(long, default_value = "config.toml")]
    config_file: PathBuf,
}

type Result<T, E = error::Error> = std::result::Result<T, E>;

fn parse_packet(packet: &pcap::Packet) -> MacAddress {
    let packet = etherparse::SlicedPacket::from_ethernet(packet.data).unwrap();
    if let Some(etherparse::LinkSlice::Ethernet2(eth_header)) = packet.link {
        MacAddress::new(eth_header.source().try_into().unwrap())
    } else {
        panic!("Unknown packet: {:2X?}", packet);
    }
}

fn run() -> Result<()> {
    let opt = Opt::from_args();
    let config = config::Config::from_file(opt.config_file)?;

    let device = pcap::Device::lookup()?;
    println!("Opening device {}", device.name);
    let mut cap = pcap::Capture::from_device(device)?.promisc(true).open()?;
    cap.filter("udp and port bootpc")?;

    let client = telegram::Client::new(&config.bot_token);

    loop {
        let packet = cap.next()?;
        let mac_address = parse_packet(&packet);
        if let Some(notification) = config.rules.get(&mac_address) {
            println!(
                "Got packet from {} ({}), notifying {}",
                notification.name, mac_address, notification.subscriber_name
            );
            if let Err(err) =
                telegram::Message::new(notification.chat_id, notification.to_string()).send(&client)
            {
                println!("Error sending Telegram message: {}", err);
            }
        } else {
            println!("Got packet from unknown MAC {}", mac_address);
        }
    }
}

fn main() {
    if let Err(err) = run() {
        eprintln!("Error: {}", err);
        std::process::exit(1);
    }
}
