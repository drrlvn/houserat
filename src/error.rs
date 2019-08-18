use snafu::Snafu;
use std::path::PathBuf;

#[derive(Debug, Snafu)]
#[snafu(visibility = "pub(crate)")]
pub enum Error {
    #[snafu(display("Unknown user: {}", user))]
    UnknownUser { user: String },
    #[snafu(display("PCAP error: {}", source))]
    PcapError { source: pcap::Error },
    #[snafu(display("Config file '{}' not found: {}", path.display(), source))]
    ConfigNotFoundError {
        path: PathBuf,
        source: std::io::Error,
    },
    #[snafu(display("Invalid config: {}", source))]
    ConfigError { source: toml::de::Error },
    #[snafu(display("Invalid MAC address '{}'", value))]
    InvalidMacAddressError { value: String },
    #[snafu(display("Failed communicating with Telegram: {}", source))]
    TelegramError { source: reqwest::Error },
}

impl From<pcap::Error> for Error {
    fn from(error: pcap::Error) -> Self {
        Error::PcapError { source: error }
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
