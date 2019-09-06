use chrono::{offset::Local, DateTime, Duration};
use lazy_static::lazy_static;

lazy_static! {
    static ref DEFAULT_ICON: String = "👤".to_string();
}

#[derive(Debug)]
pub struct Metadata {
    pub name: String,
    pub icon: Option<String>,
    pub username: Option<String>,
    pub subscriber_name: String,
    pub chat_id: i64,
    last_notified: Option<DateTime<Local>>,
}

impl Metadata {
    pub fn new(
        name: String,
        icon: Option<String>,
        username: Option<String>,
        subscriber_name: String,
        chat_id: i64,
    ) -> Self {
        Self {
            name,
            icon,
            username,
            subscriber_name,
            chat_id,
            last_notified: None,
        }
    }

    pub fn should_notify(&mut self, cooldown: &Option<Duration>, now: DateTime<Local>) -> bool {
        let cooldown = match cooldown {
            Some(cooldown) => cooldown,
            None => return true,
        };
        match self.last_notified {
            Some(last_notified) => {
                let elapsed = now - last_notified;
                if elapsed >= *cooldown {
                    self.last_notified = Some(now);
                    true
                } else {
                    false
                }
            }
            None => {
                self.last_notified = Some(now);
                true
            }
        }
    }
}

impl std::fmt::Display for Metadata {
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
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_cooldown() {
        let mut notification = Metadata::new("".to_string(), None, None, "".to_string(), 0);
        let now = Local::now();
        assert!(notification.should_notify(&None, now));
        assert!(notification.should_notify(&None, now + Duration::seconds(1)))
    }

    #[test]
    fn test_cooldown() {
        let mut notification = Metadata::new("".to_string(), None, None, "".to_string(), 0);
        let cooldown = Some(Duration::seconds(5));
        let now = Local::now();
        assert!(notification.should_notify(&cooldown, now));
        assert!(!notification.should_notify(&cooldown, now + Duration::seconds(1)));
        assert!(notification.should_notify(&cooldown, now + Duration::seconds(5)));
        assert!(!notification.should_notify(&cooldown, now + Duration::seconds(6)));
        assert!(!notification.should_notify(&cooldown, now + Duration::seconds(9)));
        assert!(notification.should_notify(&cooldown, now + Duration::seconds(10)));
    }
}
