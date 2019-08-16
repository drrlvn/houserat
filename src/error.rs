use snafu::Snafu;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu]
    IoError { source: std::io::Error },
    #[snafu]
    ConfigError { source: toml::de::Error },
    #[snafu]
    TryFromMacAddressError { value: String },
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Error::IoError { source: error }
    }
}

impl From<toml::de::Error> for Error {
    fn from(error: toml::de::Error) -> Self {
        Error::ConfigError { source: error }
    }
}
