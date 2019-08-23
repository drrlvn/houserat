use crate::mac_address::MacAddress;
use lazy_static::lazy_static;
use serde::Deserialize;
use snafu::ResultExt;
use std::collections::HashMap;
use std::path::Path;

lazy_static! {
    static ref DEFAULT_ICON: String = "👤".to_string();
}

#[derive(Clone, Debug, Deserialize)]
struct User {
    name: String,
    icon: Option<String>,
    username: Option<String>,
    chat_id: Option<i64>,
    subscriber: Option<String>,
    #[serde(default)]
    devices: Vec<MacAddress>,
}

#[derive(Debug, Deserialize)]
struct ConfigData {
    bot_token: String,
    #[serde(rename = "user")]
    users: Vec<User>,
}

#[derive(Debug)]
pub struct Notification {
    pub name: String,
    pub icon: Option<String>,
    pub username: Option<String>,
    pub subscriber_name: String,
    pub chat_id: i64,
}

#[derive(Debug)]
pub struct Config {
    pub bot_token: String,
    pub rules: HashMap<MacAddress, Notification>,
}

impl std::fmt::Display for Notification {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        if self.username.is_some() {
            write!(f, "[")?;
        }
        write!(
            f,
            "{} {}",
            self.icon.as_ref().unwrap_or(&*DEFAULT_ICON),
            self.name
        )?;
        if let Some(username) = &self.username {
            write!(f, "](t.me/{})", username)?;
        }
        write!(f, " arrived")
    }
}

impl Config {
    pub fn from_file<P: AsRef<Path>>(path: P) -> crate::Result<Config> {
        let path = path.as_ref();
        let config_content = std::fs::read(path).with_context(|| crate::error::ConfigNotFound {
            path: path.to_path_buf(),
        })?;
        let config_data: ConfigData = toml::from_slice(&config_content)?;
        let users: HashMap<&String, &User> =
            config_data.users.iter().map(|u| (&u.name, u)).collect();
        let mut rules: HashMap<MacAddress, Notification> = HashMap::new();
        for user in &config_data.users {
            let subscriber = match &user.subscriber {
                Some(subscriber) => {
                    if user.devices.is_empty() {
                        return Err(crate::error::Error::NoDevices {
                            user: user.name.clone(),
                        });
                    }
                    users
                        .get(&subscriber)
                        .ok_or_else(|| unknown_user(&subscriber))?
                }
                None => {
                    if !user.devices.is_empty() {
                        return Err(crate::error::Error::NoSubscriber {
                            user: user.name.clone(),
                        });
                    }
                    continue;
                }
            };
            let chat_id = subscriber
                .chat_id
                .ok_or_else(|| crate::error::Error::MissingChatId {
                    user: subscriber.name.clone(),
                })?;
            for device in &user.devices {
                rules
                    .insert(
                        device.clone(),
                        Notification {
                            name: user.name.clone(),
                            icon: user.icon.clone(),
                            username: user.username.clone(),
                            subscriber_name: subscriber.name.clone(),
                            chat_id,
                        },
                    )
                    .map_or(Ok(()), |v| {
                        Err(crate::error::Error::DuplicateDevice {
                            device: device.clone(),
                            user: user.name.clone(),
                            orig_user: v.name,
                        })
                    })?;
            }
        }
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
