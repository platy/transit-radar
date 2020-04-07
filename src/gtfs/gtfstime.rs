use std::convert::TryInto;
use std::error::Error;
use std::fmt;
use std::ops::{Add, Sub, AddAssign, Div};
use serde::{Serialize, Serializer, de, Deserialize, Deserializer};


#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct Duration {
  seconds: i32,
}

impl Duration {
  pub fn seconds(seconds: i32) -> Duration {
    Duration {
      seconds: seconds,
    }
  }

  pub fn minutes(minutes: i32) -> Duration {
    Duration {
      seconds: minutes * 60,
    }
  }

  pub fn mins(&self) -> i32 {
    self.seconds / 60
  }
}

impl AddAssign<Duration> for Duration {
  /// Add two `duration`s
  #[inline(always)]
  fn add_assign(&mut self, rhs: Duration) {
      self.seconds += rhs.seconds;
  }
}

impl Div<i32> for Duration {
  type Output = Duration;

  /// Add two `duration`s
  #[inline(always)]
  fn div(self, rhs: i32) -> Self::Output {
      Duration::seconds(self.seconds / rhs)
  }
}

impl Serialize for Duration {
  fn serialize<S>(&self, serializer: S,) -> Result<S::Ok, S::Error>
  where
      S: Serializer,
  {
      serializer.serialize_i32(self.seconds)
  }
}

impl <'de> Deserialize<'de> for Duration {
  fn deserialize<D>(deserializer: D) -> Result<Duration, D::Error>
  where
      D: Deserializer<'de>,
  {
      let s = i32::deserialize(deserializer)?;
      Ok(Duration::seconds(s))
  }
}

/// Implementation of a local time within a day, no attempt to handle leaps, based on time-rs with the following focus:
/// * deserialisation for the formats contained in GTFS data
/// * time can go over 24 hours to enable the continuation of the day's schedule
/// * operations that are needed for this project
/// * second precision
#[derive(Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct Time {
  seconds_since_midnight: u32,
}

impl Time {
  /// # Panics
  /// On out of range input
  pub fn from_hms(hours: u32, minutes: u32, seconds: u32) -> Time {
    Time {
      seconds_since_midnight: (hours * 60 + minutes) * 60 + seconds,
    }
  }

  /// get the clock hour, it can be over 23
  fn hour(self) -> u8 {
    (self.seconds_since_midnight / 60 / 60).try_into().unwrap()
  }

  /// get the minute of the hour
  fn minute(self) -> u8 {
    ((self.seconds_since_midnight / 60) % 60).try_into().unwrap()
  }

  /// get the seconds within the minute
  fn second(self) -> u8 {
    (self.seconds_since_midnight % 60).try_into().unwrap()
  }
}

impl Add<Duration> for Time {
  type Output = Time;

  /// Add a duration to a time, never rolls over
  /// # Panics 
  /// if the duration is negative enough to roll over to yesterday
  #[inline(always)]
  fn add(self, rhs: Duration) -> Self::Output {
    let time: i64 = self.seconds_since_midnight.into();
    let duration : i64 = rhs.seconds.into();
    Time {
      seconds_since_midnight: (time + duration).try_into().unwrap(),
    }
  }
}

impl Sub<Time> for Time {
  type Output = Duration;

  /// Subtract two `Time`s, returning the `Duration` between. This assumes
  /// both `Time`s are in the same calendar day.
  #[inline(always)]
  fn sub(self, rhs: Self) -> Self::Output {
      Duration::seconds(
          self.seconds_since_midnight as i32 - rhs.seconds_since_midnight as i32,
      )
  }
}

impl fmt::Debug for Time {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
      fmt::Display::fmt(self, f)
  }
}

impl fmt::Display for Time {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
      write!(f, "{:02}:{:02}:{:02}", self.hour(), self.minute(), self.second())
  }
}

/// # String representations
/// ```rust
/// let time: Time = "0:00:00".parse()
/// let time: Time = "1:00:00".parse()
/// let time: Time = "09:00:00".parse()
/// let time: Time = "23:59:59".parse()
/// let time: Time = "25:00:00".parse()
impl std::str::FromStr for Time {
  type Err = ParseError;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    let (hh, mm, ss) = 
      if s.len() == 8 { 
        if s[2..3] != *":" || s[5..6] != *":" { return Err(ParseError::InvalidFormat) }
        (&s[0..2], &s[3..5], &s[6..8])
      } else if s.len() == 7 { 
        if s[1..2] != *":" || s[4..5] != *":" { return Err(ParseError::InvalidFormat) }
        (&s[0..1], &s[2..4], &s[5..7])
      } else {
        return Err(ParseError::InvalidFormat)
      };
    let hours: u32 = hh.parse()?;
    let minutes: u32 = mm.parse()?;
    let seconds: u32 = ss.parse()?;
    if seconds > 59 || minutes > 59 {
      Err(ParseError::TooManySecondsOrMinutes)?;
    }
    Ok(Time {
      seconds_since_midnight: hours * 60 * 60 + minutes * 60 + seconds,
    })
  }
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

impl <'de> Deserialize<'de> for Time {
  fn deserialize<D>(deserializer: D) -> Result<Time, D::Error>
  where
      D: Deserializer<'de>,
  {
    deserializer.deserialize_str(TimeVisitor)
  }
}

impl Serialize for Time {
  fn serialize<S>(&self, serializer: S,) -> Result<S::Ok, S::Error>
  where
      S: Serializer,
  {
      let s = format!("{}", self);
      serializer.serialize_str(&s)
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
  InvalidFormat,
  TooManySecondsOrMinutes,
  ParseIntError(std::num::ParseIntError),
}

impl From<std::num::ParseIntError> for ParseError {
  fn from(err: std::num::ParseIntError) -> ParseError {
    ParseError::ParseIntError(err)
  }
}

impl fmt::Display for ParseError {
  #[inline(always)]
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
   match self {
    ParseError::InvalidFormat => write!(f, "Time should use format eg. 23:59:59"),
    ParseError::TooManySecondsOrMinutes => write!(f, "Maximum minutes or seconds is 59"),
    ParseError::ParseIntError(err) => err.fmt(f),
   }
  }
}

impl Error for ParseError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Period {
  start: Time,
  end: Time,
}

impl Period {
  /// Create a new period between these 2 times
  /// # Panics
  /// if start > end
  pub fn between(start: Time, end: Time) -> Period {
    assert!(start < end);
    Period {
      start: start,
      end: end,
    }
  }

  /// returns a new period with the same end and the new start
  /// # Panics
  /// if start > end
  pub fn with_start(&self, start: Time) -> Period {
    Self::between(start, self.end)
  }

  /// Containership, inclusive of start, exclusive of end
  pub fn contains(&self, time: Time) -> bool {
    self.start <= time && time < self.end
  }

  pub fn start(&self) -> Time {
    self.start
  }

  pub fn duration(&self) -> Duration {
    self.end - self.start
  }
}

impl fmt::Display for Period {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
      write!(f, "{}-{}", self.start, self.end)
  }
}

#[cfg(test)]
mod test {
  use super::{Time, Duration};

  #[test]
  fn subtract_times() {
    assert_eq!("12:00:15".parse::<Time>().unwrap() - "12:00:00".parse::<Time>().unwrap(), Duration::seconds(15));
    assert_eq!("12:00:00".parse::<Time>().unwrap() - "12:00:15".parse::<Time>().unwrap(), Duration::seconds(-15));
    assert_eq!("12:00:15".parse::<Time>().unwrap() - "11:59:45".parse::<Time>().unwrap(), Duration::seconds(30));
  }

  #[test]
  fn parse_and_to_string() {
    assert_eq!("00:00:00".parse::<Time>().unwrap().to_string(), "00:00:00");
    assert_eq!("00:00:01".parse::<Time>().unwrap().to_string(), "00:00:01");
    assert_eq!("23:59:59".parse::<Time>().unwrap().to_string(), "23:59:59");
    assert_eq!("24:00:00".parse::<Time>().unwrap().to_string(), "24:00:00");
    assert_eq!("25:00:00".parse::<Time>().unwrap().to_string(), "25:00:00");
    assert_eq!("5:00:00".parse::<Time>().unwrap().to_string(), "05:00:00");
  }

  #[test]
  fn invalid_parses() {
    assert!("".parse::<Time>().is_err());
    assert!("%%:%%:%%".parse::<Time>().is_err());
    assert!("00:00:0".parse::<Time>().is_err());
    assert!("00:00:000".parse::<Time>().is_err());
    assert!("00:00:60".parse::<Time>().is_err());
    assert!("00:60:00".parse::<Time>().is_err());
    assert!("00100100".parse::<Time>().is_err());
  }
}
