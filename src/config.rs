use chrono::NaiveTime;
use pnet::util::MacAddr;
use serde::Deserialize;
use snafu::ResultExt;
use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::path::Path;
use std::time::Duration;

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

#[derive(Debug, Deserialize)]
struct User<'a> {
    name: &'a str,
    icon: Option<&'a str>,
    username: Option<&'a str>,
    chat_id: Option<i64>,
    subscriber: Option<&'a str>,
    #[serde(default)]
    devices: Vec<MacAddr>,
}

#[derive(Debug, Deserialize)]
struct ConfigData<'a> {
    interface: &'a str,
    bot_token: &'a str,
    #[serde(with = "humantime_serde")]
    cooldown: Option<Duration>,
    quiet_period: Option<Period>,
    #[serde(borrow, rename = "user")]
    users: Vec<User<'a>>,
}

#[derive(Debug)]
pub struct Interface {
    pub name: String,
    pub index: u32,
    pub addresses: NetworkAddresses,
}

#[derive(Debug)]
pub struct NetworkAddresses {
    pub mac: MacAddr,
    pub ip: Ipv4Addr,
}

#[derive(Debug)]
pub struct Config {
    pub interface: Interface,
    pub bot_token: String,
    pub cooldown: Option<chrono::Duration>,
    pub quiet_period: Option<Period>,
    pub rules: HashMap<MacAddr, crate::Metadata>,
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

impl NetworkAddresses {
    pub fn new(mac: MacAddr, ip: Ipv4Addr) -> NetworkAddresses {
        NetworkAddresses { mac, ip }
    }
}

impl Config {
    pub fn from_file<P: AsRef<Path>>(path: P) -> crate::Result<Config> {
        let path = path.as_ref();
        let config_content =
            std::fs::read_to_string(path).with_context(|| crate::error::ConfigNotFound {
                path: path.to_path_buf(),
            })?;
        let config_data: ConfigData = toml::from_str(&config_content)?;

        let interface = Interface::from_name(config_data.interface)?;

        let cooldown = if let Some(cooldown) = config_data.cooldown {
            Some(
                chrono::Duration::from_std(cooldown)
                    .map_err(|_e| crate::error::Error::InvalidDuration { value: cooldown })?,
            )
        } else {
            None
        };

        let users: HashMap<&str, &User> = config_data.users.iter().map(|u| (u.name, u)).collect();
        let mut rules: HashMap<MacAddr, crate::Metadata> = HashMap::new();
        for user in &config_data.users {
            let subscriber = match &user.subscriber {
                Some(subscriber) => {
                    if user.devices.is_empty() {
                        return Err(crate::error::Error::NoDevices {
                            user: user.name.into(),
                        });
                    }
                    users
                        .get(subscriber)
                        .ok_or_else(|| unknown_user(&subscriber))?
                }
                None => {
                    if !user.devices.is_empty() {
                        return Err(crate::error::Error::NoSubscriber {
                            user: user.name.into(),
                        });
                    }
                    continue;
                }
            };
            let chat_id = subscriber
                .chat_id
                .ok_or_else(|| crate::error::Error::MissingChatId {
                    user: subscriber.name.into(),
                })?;
            for device in &user.devices {
                rules
                    .insert(
                        device.clone(),
                        crate::Metadata::new(
                            user.name.into(),
                            user.icon.map(|s| s.into()),
                            user.username.map(|s| s.into()),
                            subscriber.name.into(),
                            chat_id,
                        ),
                    )
                    .map_or(Ok(()), |v| {
                        Err(crate::error::Error::DuplicateDevice {
                            device: device.clone(),
                            user: user.name.into(),
                            orig_user: v.name.into(),
                        })
                    })?;
            }
        }

        Ok(Config {
            interface,
            bot_token: config_data.bot_token.into(),
            cooldown,
            quiet_period: config_data.quiet_period,
            rules,
        })
    }
}

impl Interface {
    fn from_name(name: &str) -> crate::Result<Interface> {
        let interface = match pnet::datalink::interfaces()
            .into_iter()
            .find(|iface| iface.name == name)
        {
            Some(interface) => interface,
            None => {
                return Err(crate::error::Error::UnknownInterface {
                    interface: name.into(),
                })
            }
        };
        let mac = match interface.mac {
            Some(mac) => mac,
            None => {
                return Err(crate::error::Error::BadInterface {
                    interface: interface.name,
                })
            }
        };
        let ip = match interface
            .ips
            .into_iter()
            .find(|ip| ip.is_ipv4())
            .map(|ip| ip.ip())
        {
            Some(std::net::IpAddr::V4(ip)) => ip,
            _ => {
                return Err(crate::error::Error::BadInterface {
                    interface: interface.name,
                })
            }
        };
        Ok(Interface {
            name: interface.name,
            index: interface.index,
            addresses: NetworkAddresses::new(mac, ip),
        })
    }
}

fn unknown_user(user: &str) -> crate::error::Error {
    crate::error::Error::UnknownUser { user: user.into() }
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
