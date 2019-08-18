use crate::mac_address::MacAddress;
use serde::Deserialize;
use snafu::ResultExt;
use std::collections::HashMap;
use std::path::Path;

#[derive(Clone, Debug, Deserialize)]
struct User {
    name: String,
    username: String,
    chat_id: i64,
}

#[derive(Clone, Debug, Deserialize)]
struct Device {
    mac_address: MacAddress,
    owner: String,
    subscriber: String,
}

#[derive(Debug, Deserialize)]
struct ConfigData {
    bot_token: String,
    #[serde(rename = "user")]
    users: Vec<User>,
    #[serde(rename = "device")]
    devices: Vec<Device>,
}

#[derive(Debug)]
pub struct Notification {
    pub name: String,
    pub username: String,
    pub chat_id: i64,
}

#[derive(Debug)]
pub struct Config {
    pub bot_token: String,
    pub rules: HashMap<MacAddress, Notification>,
}

impl Config {
    pub fn from_file<P: AsRef<Path>>(path: P) -> crate::Result<Config> {
        let path = path.as_ref();
        let config_content =
            std::fs::read(path).with_context(|| crate::error::ConfigNotFoundError {
                path: path.to_path_buf(),
            })?;
        let config_data: ConfigData = toml::from_slice(&config_content)?;
        let users: HashMap<String, User> = config_data
            .users
            .into_iter()
            .map(|u| (u.name.clone(), u))
            .collect();
        let rules: HashMap<MacAddress, Notification> = config_data
            .devices
            .into_iter()
            .map(|d| {
                let owner = users.get(&d.owner).ok_or_else(|| unknown_user(&d.owner))?;
                let subscriber = users
                    .get(&d.subscriber)
                    .ok_or_else(|| unknown_user(&d.subscriber))?;
                Ok((
                    d.mac_address,
                    Notification {
                        name: owner.name.clone(),
                        username: owner.username.clone(),
                        chat_id: subscriber.chat_id,
                    },
                ))
            })
            .collect::<Result<HashMap<_, _>, crate::error::Error>>()?;
        Ok(Config {
            bot_token: config_data.bot_token,
            rules,
        })
    }
}

fn unknown_user(user: &str) -> crate::error::Error {
    crate::error::Error::UnknownUser {
        user: user.to_string(),
    }
}
