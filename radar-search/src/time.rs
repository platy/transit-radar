use std::convert::TryInto;
use std::fmt;
use std::ops::{Add, Sub};

use chrono::{Duration, NaiveTime, Timelike};
use serde::{de, ser};

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
    pub const fn from_hms(hours: u32, minutes: u32, seconds: u32) -> Self {
        Self {
            seconds_since_midnight: (hours * 60 + minutes) * 60 + seconds,
        }
    }

    pub const fn from_seconds_since_midnight(seconds: u32) -> Self {
        Self {
            seconds_since_midnight: seconds,
        }
    }

    /// get the clock hour, it can be over 23
    pub fn hour(self) -> u8 {
        (self.seconds_since_midnight / 60 / 60).try_into().unwrap()
    }

    /// get the minute of the hour
    pub fn minute(self) -> u8 {
        ((self.seconds_since_midnight / 60) % 60)
            .try_into()
            .unwrap()
    }

    /// get the seconds within the minute
    pub fn second(self) -> u8 {
        (self.seconds_since_midnight % 60).try_into().unwrap()
    }

    pub const fn seconds_since_midnight(self) -> u32 {
        self.seconds_since_midnight
    }
}

impl ser::Serialize for Time {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        self.seconds_since_midnight.serialize(serializer)
    }
}

impl<'de> de::Deserialize<'de> for Time {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        de::Deserialize::deserialize(deserializer).map(|seconds_since_midnight| Self {
            seconds_since_midnight,
        })
    }
}

impl Add<Duration> for Time {
    type Output = Self;

    /// Add a duration to a time, never rolls over
    /// # Panics
    /// if the duration is negative enough to roll over to yesterday
    fn add(self, rhs: Duration) -> Self::Output {
        let time: i64 = self.seconds_since_midnight.into();
        let duration: i64 = rhs.num_seconds();
        Self::Output {
            seconds_since_midnight: (time + duration)
                .try_into()
                .expect("duration not to be negative enough to roll over to yesterday"),
        }
    }
}

impl Sub<Time> for Time {
    type Output = Duration;

    /// Subtract two `Time`s, returning the `Duration` between. This assumes
    /// both `Time`s are in the same calendar day.
    fn sub(self, rhs: Self) -> Self::Output {
        let lhs: i64 = self.seconds_since_midnight.try_into().unwrap();
        let rhs: i64 = rhs.seconds_since_midnight.try_into().unwrap();
        Duration::seconds(lhs - rhs)
    }
}

impl From<Time> for NaiveTime {
    fn from(time: Time) -> Self {
        Self::from_num_seconds_from_midnight_opt(time.seconds_since_midnight, 0).unwrap()
    }
}

impl From<NaiveTime> for Time {
    fn from(time: NaiveTime) -> Self {
        Self::from_seconds_since_midnight(time.num_seconds_from_midnight())
    }
}

impl fmt::Debug for Time {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl fmt::Display for Time {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:02}:{:02}:{:02}",
            self.hour(),
            self.minute(),
            self.second()
        )
    }
}

/// A period between 2 Times on the same day
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Period {
    start: Time,
    end: Time,
}

impl Period {
    /// Create a new period between these 2 times
    /// # Panics
    /// if start > end
    pub fn between(start: Time, end: Time) -> Self {
        assert!(start < end);
        Self { start, end }
    }

    /// returns a new period with the same end and the new start
    /// # Panics
    /// if start > end
    pub fn with_start(self, start: Time) -> Self {
        Self::between(start, self.end)
    }

    /// Containership, inclusive of start, exclusive of end
    pub fn contains(self, time: Time) -> bool {
        self.start <= time && time < self.end
    }

    pub const fn start(self) -> Time {
        self.start
    }

    pub fn duration(self) -> Duration {
        self.end - self.start
    }
}

impl std::ops::RangeBounds<Time> for Period {
    fn start_bound(&self) -> std::ops::Bound<&Time> {
        std::ops::Bound::Included(&self.start)
    }
    fn end_bound(&self) -> std::ops::Bound<&Time> {
        std::ops::Bound::Excluded(&self.end)
    }
}

impl fmt::Display for Period {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{}", self.start, self.end)
    }
}

/// # String representations
/// ```rust
/// use radar_search::time::Time;
/// let time: Time = "0:00:00".parse().unwrap();
/// let time: Time = "1:00:00".parse().unwrap();
/// let time: Time = "09:00:00".parse().unwrap();
/// let time: Time = "23:59:59".parse().unwrap();
/// let time: Time = "25:00:00".parse().unwrap();
impl std::str::FromStr for Time {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use std::str::from_utf8;

        let s = s.as_bytes();
        let (hh, mm, ss) = if s.len() == 8 {
            if s[2] != b':' || s[5] != b':' {
                return Err(ParseError::InvalidFormat);
            }
            (&s[0..2], &s[3..5], &s[6..8])
        } else if s.len() == 7 {
            if s[1] != b':' || s[4] != b':' {
                return Err(ParseError::InvalidFormat);
            }
            (&s[0..1], &s[2..4], &s[5..7])
        } else {
            return Err(ParseError::InvalidFormat);
        };
        let hours: u32 = from_utf8(hh)?.parse()?;
        let minutes: u32 = from_utf8(mm)?.parse()?;
        let seconds: u32 = from_utf8(ss)?.parse()?;
        if seconds > 59 || minutes > 59 {
            return Err(ParseError::TooManySecondsOrMinutes);
        }
        Ok(Self {
            seconds_since_midnight: hours * 60 * 60 + minutes * 60 + seconds,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    InvalidFormat,
    TooManySecondsOrMinutes,
    ParseIntError(std::num::ParseIntError),
}

impl From<std::num::ParseIntError> for ParseError {
    fn from(err: std::num::ParseIntError) -> Self {
        Self::ParseIntError(err)
    }
}

impl std::convert::From<std::str::Utf8Error> for ParseError {
    fn from(_err: std::str::Utf8Error) -> Self {
        Self::InvalidFormat
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFormat => write!(f, "Time should use format eg. 23:59:59"),
            Self::TooManySecondsOrMinutes => write!(f, "Maximum minutes or seconds is 59"),
            Self::ParseIntError(err) => err.fmt(f),
        }
    }
}

impl std::error::Error for ParseError {}

#[cfg(test)]
mod test {
    use super::{Duration, Time};

    #[test]
    fn hms_times() {
        assert_eq!(Time::from_hms(12, 59, 59), "12:59:59".parse().unwrap());
    }

    #[test]
    fn subtract_times() {
        assert_eq!(
            "12:00:15".parse::<Time>().unwrap() - "12:00:00".parse::<Time>().unwrap(),
            Duration::seconds(15)
        );
        assert_eq!(
            "12:00:00".parse::<Time>().unwrap() - "12:00:15".parse::<Time>().unwrap(),
            Duration::seconds(-15)
        );
        assert_eq!(
            "12:00:15".parse::<Time>().unwrap() - "11:59:45".parse::<Time>().unwrap(),
            Duration::seconds(30)
        );
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
