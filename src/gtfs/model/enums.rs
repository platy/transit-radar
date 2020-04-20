pub use radar_search::search_data::RouteType;

/// • 0 (or blank): Stop (or Platform). A location where passengers board or disembark from a transit vehicle. Is called a platform when defined within a parent_station.
/// • 1: Station. A physical structure or area that contains one or more platform.
/// • 2: Entrance/Exit. A location where passengers can enter or exit a station from the street. If an entrance/exit belongs to multiple stations, it can be linked by pathways to both, but the data provider must pick one of them as parent.
/// • 3: Generic Node. A location within a station, not matching any other location_type, which can be used to link together pathways define in pathways.txt.
/// • 4: Boarding Area. A specific location on a platform, where passengers can board and/or alight vehicles.
pub type LocationType = u8;
// pub type DirectionId = u8; // 0 or 1
// type BikesAllowed = Option<u8>; // 0, 1, or 2
// type WheelchairAccessible = Option<u8>; // 0, 1, 2
// type TransferType = u8;

/// 1 - Service is available for all Mondays in the date range.
/// 0 - Service is not available for Mondays in the date range.
pub type ServiceAvailable = u8;

/// Indicates the type of transportation used on a route.
/// More options: [https://developers.google.com/transit/gtfs/reference#routestxt] and [https://developers.google.com/transit/gtfs/reference/extended-route-types]
pub mod route_type_format {
    use super::RouteType;
    use serde::{self, de, Deserialize, Deserializer};

    pub fn deserialize<'de, D>(deserializer: D) -> Result<RouteType, D::Error>
    where
        D: Deserializer<'de>,
    {
        use RouteType::*;
        u16::deserialize(deserializer).and_then(|ordinal| match ordinal {
            2 => Ok(Rail),
            3 => Ok(Bus),
            100 => Ok(RailwayService),
            109 => Ok(SuburbanRailway),
            400 => Ok(UrbanRailway),
            700 => Ok(BusService),
            900 => Ok(TramService),
            1000 => Ok(WaterTransportService),
            num => Err(de::Error::custom(format!("Unknown route type : {}", num))),
        })
    }
}
