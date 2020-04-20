pub mod option_duration_format {
  use serde::{Deserialize, Deserializer};
  use radar_search::time::*;

  pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Duration>, D::Error>
  where
      D: Deserializer<'de>,
  {
      Option::<i32>::deserialize(deserializer)
        .map(|option| option
          .map(|num| Duration::seconds(num)))
  }
}

pub mod time_format {
  use serde::{de, Deserializer};
  use radar_search::time::*;
  use std::fmt;

  pub fn deserialize<'de, D>(deserializer: D) -> Result<Time, D::Error>
  where
      D: Deserializer<'de>,
  {
    deserializer.deserialize_str(TimeVisitor)
  }

  struct TimeVisitor;
  
  impl<'de> de::Visitor<'de> for TimeVisitor {
    type Value = Time;
  
    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "time formatted eg. \"[h]h:mm:ss\"")
    }
  
    fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
      s.parse().map_err(de::Error::custom)
    }
  }
}
