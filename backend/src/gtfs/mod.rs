use serde::Deserialize;

pub mod gtfstime;
use gtfstime::Time;

type AgencyId = u16;
pub type RouteId = String;
type RouteType = u16;
pub type TripId = u64;
pub type StopId = String;
type ShapeId = u16;
type BlockId = String;
pub type ServiceId = u16;
type ZoneId = String;
type LocationType = u8;

pub type DirectionId = u8; // 0 or 1
type BikesAllowed = Option<u8>; // 0, 1, or 2
type WheelchairAccessible = Option<u8>; // 0, 1, 2

#[derive(Debug, Deserialize)]
pub struct WithTripId {
    pub trip_id: TripId,
}

#[derive(Debug, Deserialize)]
pub struct Calendar { // "service_id","monday","tuesday","wednesday","thursday","friday","saturday","sunday","start_date","end_date"
    pub service_id: ServiceId,
    monday: u8,
    // tuesday
    // wednesday
    // thursday
    // friday
    // saturday
    pub sunday: u8,
    start_date: String, // date
    end_date: String, // date
}

#[derive(Debug, Deserialize)]
pub struct Route { //"route_id","agency_id","route_short_name","route_long_name","route_type","route_color","route_text_color","route_desc"
    pub route_id: RouteId,
    agency_id: AgencyId,
    pub route_short_name: String,
    route_long_name: Option<String>,
    pub route_type: RouteType,
    route_color: Option<String>,
    route_text_color: Option<String>,
    route_desc: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Trip { // "route_id","service_id","trip_id","trip_headsign","trip_short_name","direction_id","block_id","shape_id","wheelchair_accessible","bikes_allowed"
    pub route_id: RouteId,
    pub service_id: ServiceId,
    pub trip_id: TripId,
    trip_headsign: String,
    trip_short_name: Option<String>,
    pub direction_id: DirectionId,
    block_id: Option<BlockId>,
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
    stop_headsign: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Stop { // "stop_id","stop_code","stop_name","stop_desc","stop_lat","stop_lon","location_type","parent_station","wheelchair_boarding","platform_code","zone_id"
    pub stop_id: StopId,
    stop_code: Option<String>,
    pub stop_name: String,
    stop_desc: Option<String>,
    stop_lat: f64,
    stop_lon: f64,
    location_type: LocationType,
    pub parent_station: Option<StopId>,
    wheelchair_boarding: Option<u8>,
    platform_code: Option<String>,
    zone_id: Option<ZoneId>,
}
