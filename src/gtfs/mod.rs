use std::cmp::Ord;
use std::fmt;
use serde::{Deserialize, Serialize};

pub mod gtfstime;
pub mod db;
use gtfstime::{Time, Duration};

type AgencyId = u16;
pub type RouteId = u32;
pub type RouteType = u16;
pub type TripId = u64;
pub type StopId = u64;
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
pub struct WithTripId {
    pub trip_id: TripId,
}

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
    // start_date: String, // date
    // end_date: String, // date
}

#[derive(Copy, Clone, Eq, PartialEq, Hash, Serialize)]
pub enum Day {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

impl Calendar {
    fn days(&self) -> Vec<Day> {
        let mut days = vec![];
        for (day, val) in [Day::Monday, Day::Tuesday].iter().zip([self.monday, self.tuesday].iter()) {
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

#[derive(Debug, Serialize, Deserialize, Ord, PartialOrd, Eq, PartialEq)]
pub struct Route { //"route_id","agency_id","route_short_name","route_long_name","route_type","route_color","route_text_color","route_desc"
    #[serde(with = "route_id_format")]
    pub route_id: RouteId,
    agency_id: AgencyId,
    pub route_short_name: String,
    // route_long_name: Option<String>,
    pub route_type: RouteType,
    // route_color: Option<String>,
    // route_text_color: Option<String>,
    // route_desc: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Trip { // "route_id","service_id","trip_id","trip_headsign","trip_short_name","direction_id","block_id","shape_id","wheelchair_accessible","bikes_allowed"
    #[serde(with = "route_id_format")]
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

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize, Clone)]
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

#[derive(Debug, Serialize, Deserialize)]
pub struct Transfer { // "from_stop_id","to_stop_id","transfer_type","min_transfer_time","from_route_id","to_route_id","from_trip_id","to_trip_id"
    pub from_stop_id: StopId,
    pub to_stop_id: StopId,
    transfer_type: TransferType,
    pub min_transfer_time: Option<Duration>,
    #[serde(with = "route_id_option_format")]
    from_route_id: Option<RouteId>,
    #[serde(with = "route_id_option_format")]
    to_route_id: Option<RouteId>,
    from_trip_id: Option<TripId>,
    to_trip_id: Option<TripId>,
}

impl Stop {
    pub fn fake() -> Stop {
        Stop {
            stop_id: 0,
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
/// Maybe I can remove both of these if i use a newtype?
mod route_id_format {
    use std::convert::TryInto;
    use serde::{self, Deserializer, Serializer};
    use super::RouteId;

    pub fn serialize<S>(
        route_id: &RouteId,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u32(*route_id)
    }
    
    pub fn deserialize<'de, D>(
        deserializer: D,
    ) -> Result<RouteId, D::Error>
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
                string.split("_").next().unwrap().parse().map(|v| v).map_err(|e| serde::de::Error::custom(e))
            }

            fn visit_u32<E>(self, num: u32) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(num)
            }

            fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(v.try_into().map_err(|e| serde::de::Error::custom(e))?)
            }
            fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(v.try_into().map_err(|e| serde::de::Error::custom(e))?)
            }
        }

        deserializer.deserialize_any(StringOrInt)   
    }
}

mod route_id_option_format {
    use std::convert::TryInto;
    use serde::{self, Deserializer, Serializer};
    use super::RouteId;

    pub fn serialize<S>(
        route_id: &Option<RouteId>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if let Some(route_id) = route_id {
            serializer.serialize_some(&route_id)
        } else {
            serializer.serialize_none()
        }
    }

    pub fn deserialize<'de, D>(
        deserializer: D,
    ) -> Result<Option<RouteId>, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct StringOrInt;
                    
        impl<'de> serde::de::Visitor<'de> for StringOrInt
        {
            type Value = Option<u32>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("string or int")
            }

            fn visit_str<E>(self, string: &str) -> Result<Option<u32>, E>
            where
                E: serde::de::Error,
            {
                string.split("_").next().unwrap().parse().map(|v| Some(v)).map_err(|e| serde::de::Error::custom(e))
            }

            fn visit_u32<E>(self, num: u32) -> Result<Option<u32>, E>
            where
                E: serde::de::Error,
            {
                Ok(Some(num))
            }

            fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(Some(v.try_into().map_err(|e| serde::de::Error::custom(e))?))
            }
            fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(Some(v.try_into().map_err(|e| serde::de::Error::custom(e))?))
            }

            fn visit_none<E>(self) -> Result<Self::Value, E>
            where
            E: serde::de::Error,
            {
                Ok(None)
            }
        
            fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
            where
                D: Deserializer<'de>,
            {
                deserializer.deserialize_any(self)
            }
        }

        deserializer.deserialize_option(StringOrInt)   
    }


}
