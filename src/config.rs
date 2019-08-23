use crate::mac_address::MacAddress;
use chrono::NaiveTime;
use lazy_static::lazy_static;
use serde::Deserialize;
use snafu::ResultExt;
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

lazy_static! {
    static ref DEFAULT_ICON: String = "👤".to_string();
}

pub fn deserialize_naivetime<'de, D>(d: D) -> Result<NaiveTime, D::Error>
where
    D: serde::de::Deserializer<'de>,
{
    struct V;

    impl<'de2> serde::de::Visitor<'de2> for V {
        type Value = NaiveTime;

        fn expecting(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
            fmt.write_str("a naive time")
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            NaiveTime::parse_from_str(v, "%H:%M")
                .map_err(|_| E::invalid_value(serde::de::Unexpected::Str(v), &self))
        }
    }

    d.deserialize_str(V)
}

#[derive(Debug, Deserialize)]
pub struct Period {
    #[serde(deserialize_with = "deserialize_naivetime")]
    start: NaiveTime,
    #[serde(deserialize_with = "deserialize_naivetime")]
    end: NaiveTime,
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
    #[serde(with = "humantime_serde")]
    cooldown: Option<Duration>,
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
    pub cooldown: Option<chrono::Duration>,
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
    pub fn is_between(&self, time: NaiveTime) -> bool {
        if self.start <= self.end {
            time >= self.start && time <= self.end
        } else {
            time >= self.start || time <= self.end
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

        let cooldown = if let Some(cooldown) = config_data.cooldown {
            Some(
                chrono::Duration::from_std(cooldown)
                    .map_err(|_e| crate::error::Error::InvalidDuration { value: cooldown })?,
            )
        } else {
            None
        };

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
            cooldown,
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

    fn to_naivetime(s: &str) -> NaiveTime {
        NaiveTime::parse_from_str(s, "%H:%M").unwrap()
    }

    #[test]
    fn test_period() {
        let now = to_naivetime("23:30");
        let period1 = Period {
            start: to_naivetime("23:00"),
            end: to_naivetime("06:00"),
        };
        let period2 = Period {
            start: to_naivetime("00:00"),
            end: to_naivetime("06:00"),
        };
        assert_eq!(period1.is_between(now), true);
        assert_eq!(period2.is_between(now), false);
    }
}
