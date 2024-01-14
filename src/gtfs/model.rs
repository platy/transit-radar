//! Models of data contained in static GTFS files, as defined at [https://developers.google.com/transit/gtfs/reference]
//! Documentation on this module uses excepts from that reference.

use chrono::Duration;
pub use radar_search::time::{Period, Time};
use serde::{self, Deserialize};
use std::cmp::Ord;
pub mod id;
pub use id::*;
pub mod enums;
pub use enums::*;

/// YYYMMDD
type Date = String;

/// GTFS record
/// [https://developers.google.com/transit/gtfs/reference#calendartxt]
/// Uniquely identifies a set of dates when service is available for one or more routes.
#[derive(Debug, Deserialize)]
pub struct Calendar {
    // "service_id","monday","tuesday","wednesday","thursday","friday","saturday","sunday","start_date","end_date"
    /// Each service_id value can appear at most once in a calendar.txt file.
    pub service_id: ServiceId,
    /// Indicates whether the service operates on all Mondays in the date range specified by the start_date and end_date fields. Note that exceptions for particular dates may be listed in calendar_dates.txt. Valid options are:
    pub monday: ServiceAvailable,
    pub tuesday: ServiceAvailable,
    pub wednesday: ServiceAvailable,
    pub thursday: ServiceAvailable,
    pub friday: ServiceAvailable,
    pub saturday: ServiceAvailable,
    pub sunday: ServiceAvailable,
    /// Start service day for the service interval.
    pub start_date: Date,
    // /// End service day for the service interval. This service day is included in the interval.
    // end_date: Date,
}

/// GTFS record
/// [https://developers.google.com/transit/gtfs/reference#routestxt]
#[derive(Debug, Deserialize, Ord, PartialOrd, Eq, PartialEq)]
pub struct Route {
    //"route_id","agency_id","route_short_name","route_long_name","route_type","route_color","route_text_color","route_desc"
    /// Identifies a route.
    pub route_id: RouteId,
    /// Agency for the specified route. This field is required when the dataset provides data for routes from more than one agency in agency.txt, otherwise it is optional.
    agency_id: AgencyId,
    /// Short name of a route. This will often be a short, abstract identifier like "32", "100X", or "Green" that riders use to identify a route, but which doesn't give any indication of what places the route serves. Either route_short_name or route_long_name must be specified, or potentially both if appropriate.
    pub route_short_name: String,
    // / Full name of a route. This name is generally more descriptive than the route_short_name and often includes the route's destination or stop. Either route_short_name or route_long_name must be specified, or potentially both if appropriate.
    // route_long_name: Option<String>,
    // / Description of a route that provides useful, quality information. Do not simply duplicate the name of the route.
    // route_desc: Option<String>,
    /// Indicates the type of transportation used on a route.
    #[serde(with = "route_type_format")]
    pub route_type: RouteType,
    pub route_color: Option<String>,
    // route_text_color: Option<String>,
}

/// GTFS Record
/// [https://developers.google.com/transit/gtfs/reference#tripstxt]
#[derive(Debug, Deserialize)]
pub struct Trip {
    // "route_id","service_id","trip_id","trip_headsign","trip_short_name","direction_id","block_id","shape_id","wheelchair_accessible","bikes_allowed"
    /// Identifies a route.
    pub route_id: RouteId,
    /// Identifies a set of dates when service is available for one or more routes.
    pub service_id: ServiceId,
    /// Identifies a trip.
    pub trip_id: TripId,
    // trip_headsign: String,
    // trip_short_name: Option<String>,
    // pub direction_id: DirectionId,
    // block_id: Option<BlockId>,
    // shape_id: ShapeId,
    // wheelchair_accessible: WheelchairAccessible,
    // bikes_allowed: BikesAllowed,
}

#[derive(Debug, Deserialize)]
pub struct StopTime {
    // "trip_id","arrival_time","departure_time","stop_id","stop_sequence","pickup_type","drop_off_type","stop_headsign"
    /// Identifies a trip.
    pub trip_id: TripId,
    /// Arrival time at a specific stop for a specific trip on a route. If there are not separate times for arrival and departure at a stop, enter the same value for arrival_time and departure_time. For times occurring after midnight on the service day, enter the time as a value greater than 24:00:00 in HH:MM:SS local time for the day on which the trip schedule begins.
    /// Scheduled stops where the vehicle strictly adheres to the specified arrival and departure times are timepoints. If this stop is not a timepoint, it is recommended to provide an estimated or interpolated time. If this is not available, arrival_time can be left empty. Further, indicate that interpolated times are provided with timepoint=0. If interpolated times are indicated with timepoint=0, then time points must be indicated with timepoint=1. Provide arrival times for all stops that are time points. An arrival time must be specified for the first and the last stop in a trip.
    #[serde(with = "crate::gtfs::time::time_format")]
    pub arrival_time: Time,
    /// Departure time from a specific stop for a specific trip on a route. For times occurring after midnight on the service day, enter the time as a value greater than 24:00:00 in HH:MM:SS local time for the day on which the trip schedule begins. If there are not separate times for arrival and departure at a stop, enter the same value for arrival_time and departure_time. See the arrival_time description for more details about using timepoints correctly.
    /// The departure_time field should specify time values whenever possible, including non-binding estimated or interpolated times between timepoints.
    #[serde(with = "crate::gtfs::time::time_format")]
    pub departure_time: Time,
    /// Identifies the serviced stop. All stops serviced during a trip must have a record in stop_times.txt. Referenced locations must be stops, not stations or station entrances. A stop may be serviced multiple times in the same trip, and multiple trips and routes may service the same stop.
    pub stop_id: StopId,
    /// Order of stops for a particular trip. The values must increase along the trip but do not need to be consecutive.
    pub stop_sequence: u8,
    // pickup_type: u16,
    // drop_off_type: u16,
    // stop_headsign: Option<String>,
}

/// GTFS Record
/// [https://developers.google.com/transit/gtfs/reference#stopstxt]
///
#[derive(Debug, Deserialize, Clone)]
pub struct Stop {
    // "stop_id","stop_code","stop_name","stop_desc","stop_lat","stop_lon","location_type","parent_station","wheelchair_boarding","platform_code","zone_id"
    /// Identifies a stop, station, or station entrance.
    /// The term "station entrance" refers to both station entrances and station exits. Stops, stations or station entrances are collectively referred to as locations. Multiple routes may use the same stop.
    pub stop_id: StopId,
    // stop_code: Option<String>,
    /// Name of the location. Use a name that people will understand in the local and tourist vernacular.
    /// When the location is a boarding area (location_type=4), the stop_name should contains the name of the boarding area as displayed by the agency. It could be just one letter (like on some European intercity railway stations), or text like “Wheelchair boarding area” (NYC’s Subway) or “Head of short trains” (Paris’ RER).
    /// Conditionally Required:
    /// • Required for locations which are stops (location_type=0), stations (location_type=1) or entrances/exits (location_type=2).
    /// • Optional for locations which are generic nodes (location_type=3) or boarding areas (location_type=4).
    pub stop_name: String,
    // stop_desc: Option<String>,
    /// Latitude of the location.
    /// Conditionally Required:
    /// • Required for locations which are stops (location_type=0), stations (location_type=1) or entrances/exits (location_type=2).
    /// • Optional for locations which are generic nodes (location_type=3) or boarding areas (location_type=4).
    pub stop_lat: f64,
    /// Longitude of the location.
    /// Conditionally Required:
    /// • Required for locations which are stops (location_type=0), stations (location_type=1) or entrances/exits (location_type=2).
    /// • Optional for locations which are generic nodes (location_type=3) or boarding areas (location_type=4).
    pub stop_lon: f64,
    /// Type of the location
    pub location_type: LocationType,
    /// Defines hierarchy between the different locations defined in stops.txt. It contains the ID of the parent location, as followed:
    /// • Stop/platform (location_type=0): the parent_station field contains the ID of a station.
    /// • Station (location_type=1): this field must be empty.
    /// • Entrance/exit (location_type=2) or generic node (location_type=3): the parent_station field contains the ID of a station (location_type=1)
    /// • Boarding Area (location_type=4): the parent_station field contains ID of a platform.
    /// Conditionally Required:
    /// • Required for locations which are entrances (location_type=2), generic nodes (location_type=3) or boarding areas (location_type=4).
    /// • Optional for stops/platforms (location_type=0).
    /// • Forbidden for stations (location_type=1).
    pub parent_station: Option<StopId>,
    // wheelchair_boarding: Option<u8>,
    // platform_code: Option<String>,
    pub zone_id: Option<ZoneId>,
}

/// GTFS Record
/// [https://developers.google.com/transit/gtfs/reference#transferstxt]
/// When calculating an itinerary, GTFS-consuming applications interpolate transfers based on allowable time and stop proximity. Transfers.txt specifies additional rules and overrides for selected transfers.
#[derive(Debug, Deserialize)]
pub struct Transfer {
    // "from_stop_id","to_stop_id","transfer_type","min_transfer_time","from_route_id","to_route_id","from_trip_id","to_trip_id"
    /// Identifies a stop or station where a connection between routes begins. If this field refers to a station, the transfer rule applies to all its child stops.
    pub from_stop_id: StopId,
    /// Identifies a stop or station where a connection between routes ends. If this field refers to a station, the transfer rule applies to all child stops.
    pub to_stop_id: StopId,
    // / Indicates the type of connection for the specified (from_stop_id, to_stop_id) pair. Valid options are:
    // transfer_type: TransferType,
    /// Amount of time, in seconds, that must be available to permit a transfer between routes at the specified stops. The min_transfer_time should be sufficient to permit a typical rider to move between the two stops, including buffer time to allow for schedule variance on each route.
    #[serde(with = "crate::gtfs::time::option_duration_format")]
    pub min_transfer_time: Option<Duration>,
    // /// Non standard VBB field
    // from_route_id: Option<RouteId>,
    // /// Non standard VBB field
    // to_route_id: Option<RouteId>,
    // /// Non standard VBB field
    // from_trip_id: Option<TripId>,
    // /// Non standard VBB field
    // to_trip_id: Option<TripId>,
}

impl Stop {
    /// Position as a geo::Point
    pub fn position(&self) -> geo::Point<f64> {
        geo::Point::new(self.stop_lat, self.stop_lon)
    }

    /// Id of the parent station or own ID if this is a station
    pub fn station_id(&self) -> &StopId {
        self.parent_station.as_ref().unwrap_or(&self.stop_id)
    }
}

impl PartialEq for Stop {
    fn eq(&self, rhs: &Self) -> bool {
        self.stop_id == rhs.stop_id
    }
}

impl Eq for Stop {}

impl PartialOrd for Stop {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Stop {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.stop_id.cmp(&other.stop_id)
    }
}
