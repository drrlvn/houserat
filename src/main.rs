use config::NetworkAddresses;
use crossbeam_channel::select;
use metadata::Metadata;
use network::Event;
use pnet::util::MacAddr;
use std::collections::HashMap;
use std::path::PathBuf;
use structopt::StructOpt;
use strum_macros::Display;

mod config;
mod error;
mod metadata;
mod network;
mod telegram;

const TICK_SECS: u32 = 20;
const ALLOWED_PACKETS_LOST: u32 = 3;

#[derive(Debug, structopt::StructOpt)]
#[structopt(about)]
struct Opt {
    #[structopt(long, default_value = "config.toml")]
    config_file: PathBuf,
}

type Result<T, E = error::Error> = std::result::Result<T, E>;

#[derive(Debug, Display)]
#[strum(serialize_all = "kebab_case")]
enum Status {
    Arrived,
    Left,
}

#[derive(Debug)]
struct Tracking {
    ip: std::net::Ipv4Addr,
    outstanding: u32,
}

struct HouseRat {
    network_addresses: NetworkAddresses,
    socket: network::Socket,
    client: telegram::Client,
    cooldown: Option<chrono::Duration>,
    quiet_period: Option<config::Period>,
    rules: HashMap<MacAddr, Metadata>,
    online: HashMap<MacAddr, Tracking>,
}

impl HouseRat {
    fn new(config: config::Config) -> Result<Self> {
        Ok(Self {
            network_addresses: config.interface.addresses,
            socket: network::Socket::new(config.interface.index)?,
            client: telegram::Client::new(&config.bot_token),
            cooldown: config.cooldown,
            quiet_period: config.quiet_period,
            rules: config.rules,
            online: HashMap::new(),
        })
    }

    fn run(&mut self, mut capture: pcap::Capture<pcap::Active>) -> Result<()> {
        let (s, r) = crossbeam_channel::unbounded();

        std::thread::spawn(move || loop {
            match capture.next() {
                Ok(packet) => {
                    if let Err(e) = s.send(network::parse_packet(packet.data)) {
                        println!("Failed to send event, exiting: {}", e);
                        return;
                    }
                }
                Err(e) => {
                    println!("Failed to read packet, exiting: {}", e);
                    return;
                }
            };
        });

        let clock = crossbeam_channel::tick(std::time::Duration::from_secs(TICK_SECS.into()));

        loop {
            select! {
                recv(r) -> event => match event? {
                    Event::Connected(mac) => {
                        if self.online.contains_key(&mac) {
                            println!("Device {} reconnected, skipping notification", mac);
                        } else {
                            self.notify(mac, Status::Arrived);
                        }
                    }
                    Event::Announced { mac, ip } => {
                        if self.rules.contains_key(&mac) {
                            let _ = self.online.insert(mac, Tracking { ip, outstanding: 0 });
                        }
                    }
                    Event::Responded(mac) => {
                        if let Some(tracking) = self.online.get_mut(&mac) {
                            println!("Got keepalive response from {}", mac);
                            tracking.outstanding = 0;
                        }
                    }
                    Event::Ignored => (),
                },
                recv(clock) -> _ => {
                    let mut left = Vec::new();
                    for (mac, tracking) in &mut self.online {
                        if tracking.outstanding < ALLOWED_PACKETS_LOST {
                            println!("Sending keepalive to {} ({}), outstanding: {}", tracking.ip, mac, tracking.outstanding);
                            match self.socket.send_arp_request(&self.network_addresses, &NetworkAddresses::new(*mac, tracking.ip)) {
                                Ok(()) => tracking.outstanding += 1,
                                Err(e) => println!("Failed to send keepalive: {}", e),
                            }
                        } else {
                            println!("Assuming {} left after not receiving response for {} seconds", mac, tracking.outstanding * TICK_SECS);
                            left.push(*mac);
                        }
                    }
                    for mac in left {
                        let _ = self.online.remove(&mac);
                        self.notify(mac, Status::Left);
                    }
                }
            }
        }
    }

    fn notify(&mut self, mac: MacAddr, status: Status) {
        let metadata = match self.rules.get_mut(&mac) {
            Some(metadata) => metadata,
            None => {
                println!("Unknown MAC {} connected, ignoring", mac);
                return;
            }
        };

        let now = chrono::Local::now();

        if !metadata.should_notify(&self.cooldown, now) {
            println!(
                "{} ({}) {} during cooldown, ignoring",
                metadata.name, mac, status
            );
            return;
        }

        let is_quiet = match &self.quiet_period {
            Some(quiet_period) => quiet_period.is_between(now.naive_local().time()),
            None => false,
        };

        println!(
            "{} ({}) {}, notifying {} {}",
            metadata.name,
            mac,
            status,
            metadata.subscriber_name,
            if is_quiet { "quietly" } else { "loudly" }
        );

        if let Err(err) = telegram::Message::new(
            metadata.chat_id,
            format!("{} {}", metadata, status),
            is_quiet,
        )
        .send(&self.client)
        {
            println!("Error sending Telegram message: {}", err);
        }
    }
}

fn run() -> Result<()> {
    let opt = Opt::from_args();
    let config = config::Config::from_file(opt.config_file)?;

    println!("Listening on interface {}...", config.interface.name);
    let device = pcap::Device {
        name: config.interface.name.clone(),
        desc: None,
    };
    let mut capture = pcap::Capture::from_device(device)?.promisc(true).open()?;
    capture.direction(pcap::Direction::In)?;
    capture.filter("arp or (udp and port bootpc)")?;

    let mut houserat = HouseRat::new(config)?;
    houserat.run(capture)
}

fn main() {
    if let Err(err) = run() {
        eprintln!("Error: {}", err);
        std::process::exit(1);
    }
}
