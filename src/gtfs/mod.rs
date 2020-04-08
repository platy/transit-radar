use std::cmp::Ord;
use std::fmt;
use serde::{self, de, Deserialize, Deserializer};
use std::convert::TryInto;

pub mod gtfstime;
pub mod db;
use gtfstime::{Time, Duration};

type AgencyId = u16;

#[derive(Debug, Ord, PartialOrd, Eq, PartialEq, Hash, Clone, Copy)]
pub struct RouteId(u32);
pub type RouteType = u16;
pub type TripId = u64;
#[derive(Debug, Ord, PartialOrd, Eq, PartialEq, Hash, Clone, Copy)]
pub struct StopId(u64);
// pub type StopId = u64;
type ShapeId = u16;
// type BlockId = String;
pub type ServiceId = u16;
// type ZoneId = String;
type LocationType = u8;

pub type DirectionId = u8; // 0 or 1
type BikesAllowed = Option<u8>; // 0, 1, or 2
type WheelchairAccessible = Option<u8>; // 0, 1, 2
type TransferType = u8;

#[derive(Debug, Deserialize)]
pub struct Calendar { // "service_id","monday","tuesday","wednesday","thursday","friday","saturday","sunday","start_date","end_date"
    pub service_id: ServiceId,
    pub monday: u8,
    pub tuesday: u8,
    pub wednesday: u8,
    pub thursday: u8,
    pub friday: u8,
    pub saturday: u8,
    pub sunday: u8,
    start_date: String, // date
    // end_date: String, // date
}

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub enum Day {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

impl StopId {
    pub fn into_inner(self) -> u64 {
        self.0
    }
}

impl Calendar {
    fn days(&self) -> Vec<Day> {
        let mut days = vec![];
        for (day, val) in [Day::Monday, Day::Tuesday, Day::Wednesday, Day::Thursday, Day::Friday, Day::Saturday, Day::Sunday].iter()
                     .zip([self.monday, self.tuesday, self.wednesday, self.thursday, self.friday, self.saturday, self.sunday].iter()) {
            if *val > 0 {
                days.push(*day);
            }
        }
        days
    }
}

impl std::fmt::Display for Day {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Day::Monday => "mon",
            Day::Tuesday => "tue",
            Day::Wednesday => "wed",
            Day::Thursday => "thu",
            Day::Friday => "fri",
            Day::Saturday => "sat",
            Day::Sunday => "sun",
        })
    }
}

#[derive(Debug, Deserialize, Ord, PartialOrd, Eq, PartialEq)]
pub struct Route { //"route_id","agency_id","route_short_name","route_long_name","route_type","route_color","route_text_color","route_desc"
    pub route_id: RouteId,
    agency_id: AgencyId,
    pub route_short_name: String,
    // route_long_name: Option<String>,
    pub route_type: RouteType,
    // route_color: Option<String>,
    // route_text_color: Option<String>,
    // route_desc: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Trip { // "route_id","service_id","trip_id","trip_headsign","trip_short_name","direction_id","block_id","shape_id","wheelchair_accessible","bikes_allowed"
    pub route_id: RouteId,
    pub service_id: ServiceId,
    pub trip_id: TripId,
    // trip_headsign: String,
    // trip_short_name: Option<String>,
    pub direction_id: DirectionId,
    // block_id: Option<BlockId>,
    shape_id: ShapeId,
    wheelchair_accessible: WheelchairAccessible,
    bikes_allowed: BikesAllowed,
}

#[derive(Debug, Deserialize)]
pub struct StopTime { // "trip_id","arrival_time","departure_time","stop_id","stop_sequence","pickup_type","drop_off_type","stop_headsign"
    pub trip_id: TripId,
    pub arrival_time: Time,
    pub departure_time: Time,
    pub stop_id: StopId,
    pub stop_sequence: u32,
    pickup_type: u16,
    drop_off_type: u16,
    // stop_headsign: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Stop { // "stop_id","stop_code","stop_name","stop_desc","stop_lat","stop_lon","location_type","parent_station","wheelchair_boarding","platform_code","zone_id"
    pub stop_id: StopId,
    // stop_code: Option<String>,
    pub stop_name: String,
    // stop_desc: Option<String>,
    stop_lat: f64,
    stop_lon: f64,
    location_type: LocationType,
    pub parent_station: Option<StopId>,
    wheelchair_boarding: Option<u8>,
    // platform_code: Option<String>,
    // zone_id: Option<ZoneId>,
}

#[derive(Debug, Deserialize)]
pub struct Transfer { // "from_stop_id","to_stop_id","transfer_type","min_transfer_time","from_route_id","to_route_id","from_trip_id","to_trip_id"
    pub from_stop_id: StopId,
    pub to_stop_id: StopId,
    transfer_type: TransferType,
    pub min_transfer_time: Option<Duration>,
    from_route_id: Option<RouteId>,
    to_route_id: Option<RouteId>,
    from_trip_id: Option<TripId>,
    to_trip_id: Option<TripId>,
}

impl Stop {
    pub fn fake() -> Stop {
        Stop {
            stop_id: StopId(0),
            location_type: 0,
            parent_station: None,
            stop_lat: 0.0,
            stop_lon: 0.0,
            stop_name: "Fake stop".into(),
            wheelchair_boarding: None,
        }
    }

    pub fn position(&self) -> geo::Point<f64> {
        geo::Point::new(self.stop_lat, self.stop_lon)
    }

    pub fn station_id(&self) -> StopId {
        self.parent_station.unwrap_or(self.stop_id)
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
                Ok(RouteId(v.try_into().map_err(|e| serde::de::Error::custom(e))?))
            }
            fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(RouteId(v.try_into().map_err(|e| serde::de::Error::custom(e))?))
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
