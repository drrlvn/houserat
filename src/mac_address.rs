use serde::Deserialize;
use std::array::TryFromSliceError;
use std::convert::{TryFrom, TryInto};
use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct MacAddress([u8; 6]);

impl MacAddress {
    pub fn from_slice(data: &[u8]) -> Result<MacAddress, TryFromSliceError> {
        Ok(MacAddress(data.try_into()?))
    }
}

impl TryFrom<String> for MacAddress {
    type Error = crate::error::Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let mut nums = value.split(':').map(|n| u8::from_str_radix(n, 16));
        let mut mac_addresses = [0u8; 6];

        for octet in &mut mac_addresses {
            *octet = if let Some(Ok(n)) = nums.next() {
                n
            } else {
                return Err(Self::Error::TryFromMacAddressError { value });
            }
        }

        if nums.next().is_some() {
            return Err(Self::Error::TryFromMacAddressError { value });
        }

        Ok(MacAddress(mac_addresses))
    }
}

impl fmt::Display for MacAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut iter = self.0.iter();
        write!(
            f,
            "{:2X}:{:2X}:{:2X}:{:2X}:{:2X}:{:2X}",
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
