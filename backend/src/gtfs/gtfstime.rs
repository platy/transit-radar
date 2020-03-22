use std::convert::TryInto;
use std::error::Error;
use std::fmt;
use std::ops::{Add, Sub, AddAssign, Div};
use serde::Deserialize;
use serde::Deserializer;


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

  pub fn mins(self) -> i32 {
    self.seconds / 60
  }

  pub fn secs(self) -> i32 {
    self.seconds
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct Time {
  seconds_since_midnight: u32,
}

impl Time {
  pub fn parse(string: &str) -> Result<Time, Box<dyn Error>> {
    let parts: Vec<&str> = string.splitn(3, ":").collect();
    if parts.len() != 3 || parts[0].len() == 0 || parts[0].len() > 2 || parts[1].len() != 2 || parts[2].len() != 2 {
      Err(ParseError::InvalidFormat)?
    }
    let hours: u32 = parts[0].parse()?;
    let minutes: u32 = parts[1].parse()?;
    let seconds: u32 = parts[2].parse()?;
    if seconds > 59 || minutes > 59 {
      Err(ParseError::TooManySecondsOrMinutes)?;
    }
    Ok(Time {
      seconds_since_midnight: hours * 60 * 60 + minutes * 60 + seconds,
    })
  }

  pub fn is_after(&self, rhs: Time) -> bool {
    self.seconds_since_midnight > rhs.seconds_since_midnight
  }

  pub fn is_before(&self, rhs: Time) -> bool {
    self.seconds_since_midnight < rhs.seconds_since_midnight
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum ParseError {
  InvalidFormat,
  TooManySecondsOrMinutes,
}

impl fmt::Display for ParseError {
  #[inline(always)]
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
   match self {
    ParseError::InvalidFormat => write!(f, "Time should use format eg. 23:59:59"),
    ParseError::TooManySecondsOrMinutes => write!(f, "Maximum minutes or seconds is 59")
   }
  }
}

impl Error for ParseError {}

impl fmt::Display for Time {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
      write!(f, "{:02}:{:02}:{:02}", self.hour(), self.minute(), self.second())
  }
}

impl <'de> Deserialize<'de> for Time {
  fn deserialize<D>(deserializer: D) -> Result<Time, D::Error>
  where
      D: Deserializer<'de>,
  {
      let s = String::deserialize(deserializer)?;
      Time::parse(&s).map_err(serde::de::Error::custom)
  }
}

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
}

#[cfg(test)]
mod test {
  use super::{Time, Duration};

  #[test]
  fn subtract_times() {
    assert_eq!(Time::parse("12:00:15").unwrap() - Time::parse("12:00:00").unwrap(), Duration::seconds(15));
    assert_eq!(Time::parse("12:00:00").unwrap() - Time::parse("12:00:15").unwrap(), Duration::seconds(-15));
    assert_eq!(Time::parse("12:00:15").unwrap() - Time::parse("11:59:45").unwrap(), Duration::seconds(30));
  }

  #[test]
  fn parse_and_to_string() {
    assert_eq!(Time::parse("00:00:00").unwrap().to_string(), "00:00:00");
    assert_eq!(Time::parse("00:00:01").unwrap().to_string(), "00:00:01");
    assert_eq!(Time::parse("23:59:59").unwrap().to_string(), "23:59:59");
    assert_eq!(Time::parse("24:00:00").unwrap().to_string(), "24:00:00");
    assert_eq!(Time::parse("25:00:00").unwrap().to_string(), "25:00:00");
    assert_eq!(Time::parse("5:00:00").unwrap().to_string(), "05:00:00");
  }

  #[test]
  fn invalid_parses() {
    assert!(Time::parse("").is_err());
    assert!(Time::parse("%%:%%:%%").is_err());
    assert!(Time::parse("00:00:0").is_err());
    assert!(Time::parse("00:00:000").is_err());
    assert!(Time::parse("00:00:60").is_err());
    assert!(Time::parse("00:60:00").is_err());
    assert!(Time::parse("00100100").is_err());
  }
}
