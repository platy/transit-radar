use std::convert::TryInto;
use std::fmt;
use std::ops::{Add, AddAssign, Div, Sub};

use serde::{de, ser, Serialize, Deserialize};

/// Duration in seconds as represented in GTFS data, used for transfers.txt
/// # Examples
/// ```rust
/// use radar_search::time::Duration;
/// assert_eq!(Duration::seconds(60), Duration::minutes(1));
/// ```
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
pub struct Duration {
    seconds: i32,
}

impl Duration {
    /// Construct a duration of a number of seconds
    pub fn seconds(seconds: i32) -> Duration {
        Duration { seconds: seconds }
    }

    /// Construct a duration of a number of minutes
    pub fn minutes(minutes: i32) -> Duration {
        Duration {
            seconds: minutes * 60,
        }
    }

    /// Convert to minutes
    pub fn to_mins(&self) -> i32 {
        self.seconds / 60
    }

    /// Convert to seconds
    pub fn to_secs(&self) -> i32 {
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
        ((self.seconds_since_midnight / 60) % 60)
            .try_into()
            .unwrap()
    }

    /// get the seconds within the minute
    fn second(self) -> u8 {
        (self.seconds_since_midnight % 60).try_into().unwrap()
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
    fn deserialize<D>(deserializer: D) -> Result<Time, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        de::Deserialize::deserialize(deserializer).map(|seconds_since_midnight| Time {
            seconds_since_midnight,
        })
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
        let duration: i64 = rhs.seconds.into();
        Time {
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
    #[inline(always)]
    fn sub(self, rhs: Self) -> Self::Output {
        Duration::seconds(self.seconds_since_midnight as i32 - rhs.seconds_since_midnight as i32)
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
    type Err = TimeParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.as_bytes();
        let (hh, mm, ss) = if s.len() == 8 {
            if s[2] != b':' || s[5] != b':' {
                return Err(TimeParseError::InvalidFormat);
            }
            (&s[0..2], &s[3..5], &s[6..8])
        } else if s.len() == 7 {
            if s[1] != b':' || s[4] != b':' {
                return Err(TimeParseError::InvalidFormat);
            }
            (&s[0..1], &s[2..4], &s[5..7])
        } else {
            return Err(TimeParseError::InvalidFormat);
        };
        use std::str::from_utf8;
        let hours: u32 = from_utf8(hh)?.parse()?;
        let minutes: u32 = from_utf8(mm)?.parse()?;
        let seconds: u32 = from_utf8(ss)?.parse()?;
        if seconds > 59 || minutes > 59 {
            Err(TimeParseError::TooManySecondsOrMinutes)?;
        }
        Ok(Time {
            seconds_since_midnight: hours * 60 * 60 + minutes * 60 + seconds,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimeParseError {
    InvalidFormat,
    TooManySecondsOrMinutes,
    ParseIntError(std::num::ParseIntError),
}

impl From<std::num::ParseIntError> for TimeParseError {
    fn from(err: std::num::ParseIntError) -> TimeParseError {
        TimeParseError::ParseIntError(err)
    }
}

impl std::convert::From<std::str::Utf8Error> for TimeParseError {
    fn from(_err: std::str::Utf8Error) -> TimeParseError {
        TimeParseError::InvalidFormat
    }
}

impl fmt::Display for TimeParseError {
    #[inline(always)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use TimeParseError::*;
        match self {
            InvalidFormat => write!(f, "Time should use format eg. 23:59:59"),
            TooManySecondsOrMinutes => write!(f, "Maximum minutes or seconds is 59"),
            ParseIntError(err) => err.fmt(f),
        }
    }
}

impl std::error::Error for TimeParseError {}

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
