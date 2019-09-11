use c_ares_resolver::Resolver;
use config::NetworkAddresses;
use crossbeam_channel::{never, select};
use metadata::Metadata;
use network::Event;
use pnet::util::MacAddr;
use std::collections::{hash_map, HashMap};
use std::path::PathBuf;
use structopt::StructOpt;

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

#[derive(Debug)]
enum Status {
    Arrived,
    Left,
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Arrived => write!(f, "arrived"),
            Self::Left => write!(f, "left"),
        }
    }
}

#[derive(Debug)]
struct Tracking {
    ip: std::net::Ipv4Addr,
    outstanding: u32,
}

struct HouseRat {
    interface_name: String,
    network_addresses: NetworkAddresses,
    socket: network::Socket,
    client: telegram::Client,
    cooldown: Option<chrono::Duration>,
    quiet_period: Option<config::Period>,
    devices: Option<Vec<config::Device>>,
    rules: HashMap<MacAddr, Metadata>,
    online: HashMap<MacAddr, Tracking>,
}

impl HouseRat {
    fn new(config: config::Config) -> Result<Self> {
        Ok(Self {
            interface_name: config.interface.name,
            network_addresses: config.interface.addresses,
            socket: network::Socket::new(config.interface.index)?,
            client: telegram::Client::new(&config.bot_token),
            cooldown: config.cooldown,
            quiet_period: config.quiet_period,
            devices: Some(config.devices),
            rules: config.rules,
            online: HashMap::new(),
        })
    }

    fn start_pcap(&mut self) -> Result<crossbeam_channel::Receiver<Event>> {
        let mut capture = pcap::Capture::from_device(self.interface_name.as_str())?
            .promisc(true)
            .open()?;
        capture.direction(pcap::Direction::In)?;
        capture.filter("arp or (udp and port bootpc)")?;

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

        Ok(r)
    }

    fn run(&mut self) -> Result<()> {
        let cap_r = self.start_pcap()?;

        let (resolve_s, resolve_r) = crossbeam_channel::unbounded();
        let resolver = Resolver::new().expect("Failed to create resolver");
        for device in self.devices.as_ref().unwrap() {
            let resolve_s2 = resolve_s.clone();
            let mac = device.mac;
            resolver.query_a(&device.hostname, move |result| match result {
                Ok(result) => {
                    for a_result in result.into_iter() {
                        if let Err(e) = resolve_s2.send((mac, a_result.ipv4())) {
                            println!("Failed to send address resolution: {}", e);
                        }
                    }
                }
                Err(e) => println!("Failed to resolve: {}", e),
            });
        }
        drop(resolve_s);
        let mut resolve_r = Some(&resolve_r);

        let clock = crossbeam_channel::tick(std::time::Duration::from_secs(TICK_SECS.into()));

        #[allow(clippy::drop_copy, clippy::zero_ptr)]
        loop {
            select! {
                recv(cap_r) -> event => self.handle_event(event?),
                recv(clock) -> _ => self.handle_clock(),
                recv(resolve_r.unwrap_or(&never())) -> device => match device {
                    Ok((mac, ip)) => self.handle_resolve(mac, ip),
                    Err(_) => {
                        resolve_r = None;
                        self.devices = None;
                    }
                },
            }
        }
    }

    fn handle_resolve(&self, mac: MacAddr, ip: std::net::Ipv4Addr) {
        println!("Resolved: {}", ip);
        if let Err(e) = self
            .socket
            .send_arp_request(&self.network_addresses, &NetworkAddresses::new(mac, ip))
        {
            println!("Failed to send ARP request to {}: {}", ip, e);
        }
    }

    fn handle_event(&mut self, event: Event) {
        match event {
            Event::Connected(mac) => {
                if self.online.contains_key(&mac) {
                    println!("Device {} reconnected, skipping notification", mac);
                } else {
                    self.notify(mac, Status::Arrived);
                }
            }
            Event::Alive { mac, ip } => {
                if self.rules.contains_key(&mac) {
                    println!("Device {} is alive", mac);
                    match self.online.entry(mac) {
                        hash_map::Entry::Occupied(mut occupied) => {
                            occupied.get_mut().outstanding = 0
                        }
                        hash_map::Entry::Vacant(vacant) => {
                            vacant.insert(Tracking { ip, outstanding: 0 });
                        }
                    }
                }
                if let Some(tracking) = self.online.get_mut(&mac) {
                    tracking.outstanding = 0;
                }
            }
            Event::Ignored => (),
        }
    }

    fn handle_clock(&mut self) {
        let mut left = Vec::new();
        for (mac, tracking) in &mut self.online {
            if tracking.outstanding < ALLOWED_PACKETS_LOST {
                println!(
                    "Sending keepalive to {} ({}), outstanding: {}",
                    tracking.ip, mac, tracking.outstanding
                );
                match self.socket.send_arp_request(
                    &self.network_addresses,
                    &NetworkAddresses::new(*mac, tracking.ip),
                ) {
                    Ok(()) => tracking.outstanding += 1,
                    Err(e) => println!("Failed to send keepalive: {}", e),
                }
            } else {
                println!(
                    "Assuming {} left after not receiving response for {} seconds",
                    mac,
                    tracking.outstanding * TICK_SECS
                );
                left.push(*mac);
            }
        }
        for mac in left {
            let _ = self.online.remove(&mac);
            self.notify(mac, Status::Left);
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

    let mut houserat = HouseRat::new(config)?;
    houserat.run()
}

fn main() {
    if let Err(err) = run() {
        eprintln!("Error: {}", err);
        std::process::exit(1);
    }
}
