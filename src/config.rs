use crate::mac_address::MacAddress;
use chrono::NaiveTime;
use lazy_static::lazy_static;
use serde::Deserialize;
use snafu::ResultExt;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::path::Path;

lazy_static! {
    static ref DEFAULT_ICON: String = "👤".to_string();
}

#[derive(Clone, Debug, Deserialize)]
#[serde(try_from = "&str")]
struct Time(NaiveTime);

impl TryFrom<&str> for Time {
    type Error = chrono::format::ParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Ok(Time(NaiveTime::parse_from_str(value, "%H:%M")?))
    }
}

#[derive(Debug, Deserialize)]
pub struct Period {
    start: Time,
    end: Time,
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
    quiet_period: Option<Period>,
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
    pub quiet_period: Option<Period>,
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

impl Period {
    pub fn is_now_between(&self) -> bool {
        let now = chrono::Local::now().naive_local().time();
        self._is_between(now)
    }

    fn _is_between(&self, time: NaiveTime) -> bool {
        if self.start.0 <= self.end.0 {
            time >= self.start.0 && time <= self.end.0
        } else {
            time >= self.start.0 || time <= self.end.0
        }
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
            quiet_period: config_data.quiet_period,
            rules,
        })
    }
}

fn unknown_user(user: &str) -> crate::error::Error {
    crate::error::Error::UnknownUser {
        user: user.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::TryInto;

    #[test]
    fn test_period() {
        let now = NaiveTime::parse_from_str("23:30", "%H:%M").unwrap();
        let period1 = Period {
            start: "23:00".try_into().unwrap(),
            end: "06:00".try_into().unwrap(),
        };
        let period2 = Period {
            start: "00:00".try_into().unwrap(),
            end: "06:00".try_into().unwrap(),
        };
        assert_eq!(period1._is_between(now), true);
        assert_eq!(period2._is_between(now), false);
    }
}
