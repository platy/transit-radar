pub mod option_duration_format {
    use serde::{Deserialize, Deserializer};

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<chrono::Duration>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Option::<i64>::deserialize(deserializer).map(|option| option.map(chrono::Duration::seconds))
    }
}

pub mod time_format {
    use radar_search::time::*;
    use serde::{de, Deserializer};
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
