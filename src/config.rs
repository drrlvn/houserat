use crate::mac_address::MacAddress;
use serde::Deserialize;
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
        let config_content = std::fs::read(path.as_ref())?;
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
                let owner = users.get(&d.owner).unwrap();
                let subscriber = users.get(&d.subscriber).unwrap();
                (
                    d.mac_address,
                    Notification {
                        name: owner.name.clone(),
                        username: owner.username.clone(),
                        chat_id: subscriber.chat_id,
                    },
                )
            })
            .collect();
        Ok(Config {
            bot_token: config_data.bot_token,
            rules,
        })
    }
}
