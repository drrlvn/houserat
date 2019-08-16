use mac_address::MacAddress;
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

fn parse_packet(packet: &pcap::Packet) -> Result<MacAddress> {
    let packet = etherparse::SlicedPacket::from_ethernet(packet.data).unwrap();
    if let Some(etherparse::LinkSlice::Ethernet2(eth_header)) = packet.link {
        Ok(MacAddress::from_slice(eth_header.source()).unwrap())
    } else {
        panic!("Unknown packet: {:2X?}", packet);
    }
}

fn run() -> Result<()> {
    let opt = Opt::from_args();
    let config = config::Config::from_file(opt.config_file)?;

    let mut cap = pcap::Capture::from_device(pcap::Device::lookup().unwrap())
        .unwrap()
        .promisc(true)
        .open()
        .unwrap();
    cap.filter("udp and port bootpc").unwrap();

    let client = telegram::Client::new(&config.bot_token);

    while let Ok(packet) = cap.next() {
        let mac_address = parse_packet(&packet)?;
        if let Some(notification) = config.rules.get(&mac_address) {
            telegram::Message::new(
                notification.chat_id,
                format!(
                    "[{}](t.me/{}) came home",
                    notification.name, notification.username
                ),
            )
            .send(&client);
        }
        println!("Sender: {:?}", mac_address);
    }

    Ok(())
}

fn main() {
    if let Err(err) = run() {
        eprintln!("Error: {}", err);
        std::process::exit(1);
    }
}
