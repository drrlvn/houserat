use serde::Deserialize;
use std::convert::TryFrom;
use std::fmt;

#[derive(Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "&str")]
pub struct MacAddress([u8; 6]);

impl MacAddress {
    pub fn new(data: [u8; 6]) -> MacAddress {
        Self(data)
    }
}

impl TryFrom<&str> for MacAddress {
    type Error = crate::error::Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let mut nums = value.split(':').map(|n| u8::from_str_radix(n, 16));
        let mut mac_addresses = [0u8; 6];

        for octet in &mut mac_addresses {
            *octet = if let Some(Ok(n)) = nums.next() {
                n
            } else {
                return Err(Self::Error::InvalidMacAddress {
                    value: value.to_string(),
                });
            }
        }

        if nums.next().is_some() {
            return Err(Self::Error::InvalidMacAddress {
                value: value.to_string(),
            });
        }

        Ok(MacAddress(mac_addresses))
    }
}

impl fmt::Display for MacAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut iter = self.0.iter();
        write!(
            f,
            "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
            iter.next().unwrap(),
            iter.next().unwrap(),
            iter.next().unwrap(),
            iter.next().unwrap(),
            iter.next().unwrap(),
            iter.next().unwrap()
        )
    }
}

impl fmt::Debug for MacAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! bad {
        ($value:expr) => {
            match MacAddress::try_from($value) {
                Ok(mac) => panic!("parsed {} to: {:#?}", $value, mac),
                Err(crate::error::Error::InvalidMacAddress { .. }) => (),
                Err(e) => panic!("failed with: {}", e),
            }
        };
    }

    fn good(value: &str) {
        MacAddress::try_from(value).unwrap();
    }

    #[test]
    fn test_try_from() {
        bad!("wat");
        good("00:01:02:03:04:05");
        good("00:01:02:03:04:0");
        bad!("00:01:02:03:04:");
        bad!("00:01:02:03:04");
        bad!(":00:01:02:03:04");
    }

    #[test]
    fn test_display() {
        let mac_string = "00:01:02:03:04:05";
        let mac = MacAddress::try_from(mac_string).unwrap();
        assert_eq!(format!("{}", mac), mac_string);
    }
}
