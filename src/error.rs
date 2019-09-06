use pnet::util::MacAddr;
use snafu::Snafu;
use std::path::PathBuf;

#[derive(Debug, Snafu)]
#[snafu(visibility = "pub(crate)")]
pub enum Error {
    #[snafu(display("Unknown interface {}", interface))]
    UnknownInterface { interface: String },
    #[snafu(display("Interface {} has no MAC or IP", interface))]
    BadInterface { interface: String },
    #[snafu(display("Unknown user {}", user))]
    UnknownUser { user: String },
    #[snafu(display("Missing chat_id for '{}'", user))]
    MissingChatId { user: String },
    #[snafu(display("User '{}' has same device {} as '{}'", user, device, orig_user))]
    DuplicateDevice {
        device: MacAddr,
        user: String,
        orig_user: String,
    },
    #[snafu(display("User '{}' has subscriber but no devices", user))]
    NoDevices { user: String },
    #[snafu(display("User '{}' has devices but no subscriber", user))]
    NoSubscriber { user: String },
    #[snafu(display("Duration {:?} is out of range", value))]
    InvalidDuration { value: std::time::Duration },
    #[snafu(display("Config file '{}' not found: {}", path.display(), source))]
    ConfigNotFound {
        path: PathBuf,
        source: std::io::Error,
    },
    #[snafu(display("Invalid config: {}", source))]
    ConfigError { source: toml::de::Error },
    #[snafu(display("PCAP error: {}", source))]
    PcapError { source: pcap::Error },
    #[snafu(display("PCAP thread exited: {}", source))]
    RecvError {
        source: crossbeam_channel::RecvError,
    },
    #[snafu(display("Failed to send ARP packet: {}", source))]
    SendError { source: std::io::Error },
    #[snafu(display("Failed communicating with Telegram: {}", source))]
    TelegramError { source: reqwest::Error },
}

impl From<pcap::Error> for Error {
    fn from(error: pcap::Error) -> Self {
        Error::PcapError { source: error }
    }
}

impl From<crossbeam_channel::RecvError> for Error {
    fn from(error: crossbeam_channel::RecvError) -> Self {
        Error::RecvError { source: error }
    }
}

impl From<toml::de::Error> for Error {
    fn from(error: toml::de::Error) -> Self {
        Error::ConfigError { source: error }
    }
}

impl From<reqwest::Error> for Error {
    fn from(error: reqwest::Error) -> Self {
        Error::TelegramError { source: error }
    }
}
