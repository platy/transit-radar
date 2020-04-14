use std::convert::TryInto;
use serde::{self, de, Deserialize, Deserializer};

pub type AgencyId = u16;
#[derive(Debug, Ord, PartialOrd, Eq, PartialEq, Hash, Clone, Copy)]
pub struct RouteId(u32);
pub type TripId = u64;
#[derive(Debug, Ord, PartialOrd, Eq, PartialEq, Hash, Clone, Copy)]
pub struct StopId(u64);
pub type ShapeId = u16;
// type BlockId = String;
pub type ServiceId = u16;
// type ZoneId = String;


impl StopId {
  pub fn into_inner(self) -> u64 {
      self.0
  }
}

impl From<u32> for RouteId {
  fn from(num: u32) -> RouteId {
    RouteId(num)
  }
}

impl From<u64> for StopId {
  fn from(num: u64) -> StopId {
    StopId(num)
  }
}


/// The VBB route id format is eg. `19105_700`, the first part seems to be unique on its own and the second part just seems to duplicate the route type, so we discard it
impl<'de> Deserialize<'de> for RouteId {
  fn deserialize<D>(deserializer: D) -> Result<RouteId, D::Error>
  where
      D: Deserializer<'de>,
  {
      struct StringOrInt;
                  
      impl<'de> serde::de::Visitor<'de> for StringOrInt
      {
          type Value = RouteId;

          fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
              formatter.write_str("string or int")
          }

          fn visit_str<E>(self, string: &str) -> Result<Self::Value, E>
          where
              E: serde::de::Error,
          {
              string.split("_").next().unwrap().parse().map(|v| RouteId(v)).map_err(|e| serde::de::Error::custom(e))
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
              self.visit_u32(v.try_into().map_err(|e| serde::de::Error::custom(e))?)
          }
          fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
          where
              E: serde::de::Error,
          {
              self.visit_u32(v.try_into().map_err(|e| serde::de::Error::custom(e))?)
          }
      }

      deserializer.deserialize_any(StringOrInt)   
  }
}

/// One of VBB's StopIds has 'D_' in front of it, I don't know why. This deserialiser strips that off and parses a u64
impl<'de> Deserialize<'de> for StopId {
  fn deserialize<D>(deserializer: D) -> Result<StopId, D::Error>
  where
      D: Deserializer<'de>,
  {
      struct StrippedLeadingGarbage;
                  
      impl<'de> serde::de::Visitor<'de> for StrippedLeadingGarbage
      {
          type Value = StopId;

          fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
              formatter.write_str("u64, possibly preceded with some garbage")
          }

          fn visit_str<E>(self, string: &str) -> Result<Self::Value, E>
          where
              E: serde::de::Error,
          {
              string.bytes().skip_while(|b| !b.is_ascii_digit()).try_fold(0u64, |mut result, b| {
                  let x: u64 = match (b as char).to_digit(10) {
                      Some(x) => x as u64,
                      None => return Err(de::Error::invalid_type(de::Unexpected::Str(string), &self)),
                  };
                  result = match result.checked_mul(10) {
                      Some(result) => result,
                      None => return Err(de::Error::invalid_value(de::Unexpected::Str(string), &self)),
                  };
                  result = match result.checked_add(x) {
                      Some(result) => result,
                      None => return Err(de::Error::invalid_value(de::Unexpected::Str(string), &self)),
                  };
                  Ok(result)
              }).map(|u| StopId(u))
          }

          fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
          where
              E: serde::de::Error,
          {
              Ok(StopId(v))
          }
          fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
          where
              E: serde::de::Error,
          {
              Ok(StopId(v.try_into().map_err(|e| serde::de::Error::custom(e))?))
          }
      }

      deserializer.deserialize_any(StrippedLeadingGarbage)   
  }
}

#[cfg(test)]
mod test_route_id {
  use super::RouteId;
  use serde_test::{Token, assert_de_tokens};

  #[test]
  fn test_0() {
      let id = RouteId(0);

      assert_de_tokens(&id, &[
          Token::U32 (0),
      ]);
  }

  #[test]
  fn test_max() {
      let id = RouteId(std::u32::MAX);

      assert_de_tokens(&id, &[
          Token::U32 (std::u32::MAX),
      ]);
  }

  #[test]
  fn test_route_type_suffix() {
      let id = RouteId(12345);

      assert_de_tokens(&id, &[
          Token::BorrowedStr ("12345_700"),
      ]);
  }
}

#[cfg(test)]
mod test_stop_id {
  use super::StopId;
  use serde_test::{Token, assert_de_tokens};

  #[test]
  fn test_0() {
      let id = StopId(0);

      assert_de_tokens(&id, &[
          Token::U64 (0),
      ]);
  }

  #[test]
  fn test_max() {
      let id = StopId(std::u64::MAX);

      assert_de_tokens(&id, &[
          Token::U64 (std::u64::MAX),
      ]);
  }

  #[test]
  fn test_garbage_prefix() {
      let id = StopId(000008003774);

      assert_de_tokens(&id, &[
          Token::BorrowedStr ("D_000008003774"),
      ]);
  }
}
