use chrono::{offset::Local, DateTime, Duration};
use std::collections::hash_map::Entry;
use std::collections::HashMap;

pub enum Hysteresis<'a> {
    Disabled,
    Enabled {
        history: HashMap<&'a str, DateTime<Local>>,
        cooldown: Duration,
    },
}

impl<'a> Hysteresis<'a> {
    pub fn new(cooldown: Option<Duration>) -> Self {
        match cooldown {
            None => Hysteresis::Disabled,
            Some(cooldown) => Hysteresis::Enabled {
                history: HashMap::new(),
                cooldown,
            },
        }
    }

    pub fn should_notify<'b: 'a>(&mut self, now: DateTime<Local>, user: &'b str) -> bool {
        match self {
            Self::Disabled => true,
            Self::Enabled { history, cooldown } => match history.entry(user) {
                Entry::Occupied(mut occupied) => {
                    let elapsed = now - *occupied.get();
                    if elapsed >= *cooldown {
                        *occupied.get_mut() = now;
                        true
                    } else {
                        false
                    }
                }
                Entry::Vacant(vacant) => {
                    vacant.insert(now);
                    true
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_cooldown() {
        let mut hysteresis = Hysteresis::new(None);
        let now = Local::now();
        assert!(hysteresis.should_notify(now, "a"));
        assert!(hysteresis.should_notify(now + Duration::seconds(1), "a"))
    }

    #[test]
    fn test_cooldown() {
        let mut hysteresis = Hysteresis::new(Some(Duration::seconds(5)));
        let now = Local::now();
        assert!(hysteresis.should_notify(now, "a"));
        assert!(!hysteresis.should_notify(now + Duration::seconds(1), "a"));
        assert!(hysteresis.should_notify(now + Duration::seconds(3), "b"));
        assert!(hysteresis.should_notify(now + Duration::seconds(5), "a"));
        assert!(!hysteresis.should_notify(now + Duration::seconds(6), "a"));
        assert!(!hysteresis.should_notify(now + Duration::seconds(7), "b"));
        assert!(hysteresis.should_notify(now + Duration::seconds(8), "b"));
        assert!(!hysteresis.should_notify(now + Duration::seconds(9), "a"));
        assert!(hysteresis.should_notify(now + Duration::seconds(10), "a"));
    }
}
