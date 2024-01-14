use serde::{self, Deserialize, Deserializer};
use std::{convert::TryInto, num::NonZeroU32};

pub type AgencyId = u16;
#[derive(Debug, Ord, PartialOrd, Eq, PartialEq, Hash, Clone, Copy)]
pub struct RouteId(u32);
pub type TripId = NonZeroU32;
pub type StopId = String;
pub type ShapeId = u16;
// type BlockId = String;
pub type ServiceId = u16;
pub type ZoneId = String;

impl From<u32> for RouteId {
    fn from(num: u32) -> RouteId {
        RouteId(num)
    }
}

impl RouteId {
    pub fn into_inner(self) -> u32 {
        self.0
    }
}

/// The VBB route id format is eg. `19105_700`, the first part seems to be unique on its own and the second part just seems to duplicate the route type, so we discard it
impl<'de> Deserialize<'de> for RouteId {
    fn deserialize<D>(deserializer: D) -> Result<RouteId, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct StringOrInt;

        impl<'de> serde::de::Visitor<'de> for StringOrInt {
            type Value = RouteId;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("string or int")
            }

            fn visit_str<E>(self, string: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                string
                    .split('_')
                    .next()
                    .unwrap()
                    .parse()
                    .map(RouteId)
                    .map_err(serde::de::Error::custom)
            }

            fn visit_u32<E>(self, num: u32) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(RouteId(num))
            }

            fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                self.visit_u32(v.try_into().map_err(serde::de::Error::custom)?)
            }
            fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                self.visit_u32(v.try_into().map_err(serde::de::Error::custom)?)
            }
        }

        deserializer.deserialize_any(StringOrInt)
    }
}

#[cfg(test)]
mod test_route_id {
    use super::RouteId;
    use serde_test::{assert_de_tokens, Token};

    #[test]
    fn test_0() {
        let id = RouteId(0);

        assert_de_tokens(&id, &[Token::U32(0)]);
    }

    #[test]
    fn test_max() {
        let id = RouteId(std::u32::MAX);

        assert_de_tokens(&id, &[Token::U32(std::u32::MAX)]);
    }

    #[test]
    fn test_route_type_suffix() {
        let id = RouteId(12345);

        assert_de_tokens(&id, &[Token::BorrowedStr("12345_700")]);
    }
}
