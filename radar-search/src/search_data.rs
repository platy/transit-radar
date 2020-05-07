use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::default::Default;
use std::fmt;

use crate::time::*;

pub type AgencyId = u16;
pub type RouteId = u32;
pub type TripId = u64;
pub type StopId = u64;
pub type ShapeId = u16;
// type BlockId = String;
pub type ServiceId = u16;
// type ZoneId = String;

/// Refers to a specific stop of a specific trip (an arrival / departure)
pub type TripStopRef = (TripId, usize); // usize refers to the index of the stop in the trip, should probably instead use stop sequence

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Clone, Copy, Serialize, Deserialize)]
pub enum Day {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
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

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Clone, Copy, Serialize, Deserialize)]
pub enum RouteType {
    Rail,                  // 2
    Bus,                   // 3
    RailwayService,        // 100
    SuburbanRailway,       // 109
    UrbanRailway,          // 400
    BusService,            // 700
    TramService,           // 900
    WaterTransportService, // 1000
}

/// Parsed and indexed GTFS data
/// * efficient lookups for searching
/// * can be used on server and client
/// * can be serialised to transfer to client
/// * can be diffed to sync differences to client
/// * diffs only contain additions which can be refferrd by id's in each of the maps
///
/// Routes: has all
/// Trips: can have just those relevent to the performed searches
/// Stops: can have just those visited on the searched trips + transfers
/// Departures are stored on the stops and reference the stops within the trips that are present, they are not synced but rather are cross references added when the trips are added, they are present when their trip is present
///
/// This could still be a lot of data, a friedrichstrasse search for 30 mins with all modes could include 213 trips and more than 1000 stops. But it still doesn't sound like more than a meg. And prioritisng the sync so that something useful shows fast could be very interesting
#[derive(Serialize, Deserialize)]
pub struct GTFSData {
    // sync whole trip as unit
    pub (crate) trips: HashMap<TripId, Trip>,
    pub (crate) stops: HashMap<StopId, Stop>,

    // all synced initially
    services_by_day: HashMap<Day, HashSet<ServiceId>>,
    timetable_start_date: String,
}

impl<'r> GTFSData {
    pub fn builder(
        services_by_day: HashMap<Day, HashSet<ServiceId>>,
        timetable_start_date: String,
    ) -> Builder {
        Builder {
            data: GTFSData {
                services_by_day,
                timetable_start_date,
                stops: HashMap::new(),
                trips: HashMap::new(),
            },
            stop_children: HashMap::new(),
            routes: HashMap::new(),
            departure_count: 0,
        }
    }

    pub fn build_from(&'r self) -> FilterBuilder<'r> {
        FilterBuilder {
            new_data: GTFSData {
                services_by_day: self.services_by_day.clone(),
                timetable_start_date: self.timetable_start_date.clone(),
                stops: HashMap::new(),
                trips: HashMap::new(),
            },
            existing_data: &self,
        }
    }

    /// Start date of the timetable based upon the calendar records
    pub fn timetable_start_date(&self) -> &str {
        &self.timetable_start_date
    }

    /// Get the route that the specified trip is a part of
    pub fn get_route_for_trip(&self, trip_id: &TripId) -> &Route {
        self.trips
            .get(trip_id)
            .map(|trip| &trip.route)
            .expect("To have referenced trip")
    }

    /// Get all the services which run on a particular day of the week
    pub fn services_of_day(&self, day: Day) -> HashSet<ServiceId> {
        self.services_by_day
            .get(&day)
            .cloned()
            .unwrap_or(HashSet::new())
    }

    /// finds all trips leaving a stop within a time period, using the provided services, includes the stop time for that stop and all following stops
    pub fn trips_from(
        &self,
        stop: &Stop,
        services: &HashSet<ServiceId>,
        period: Period,
    ) -> Vec<(&Trip, &[StopTime])> {
        let departures = stop.departures(period);
        departures
            .into_iter()
            .filter_map(move |stop_ref: &TripStopRef| {
                let &(trip_id, _sequence) = stop_ref;
                if let Some(trip) = self.trips.get(&trip_id) {
                    if services.contains(&trip.service_id) {
                        return Some((trip, self.stop_times(stop_ref)));
                    }
                }
                None
            })
            .collect()
    }

    pub fn get_stop(&self, id: &StopId) -> Option<&Stop> {
        self.stops.get(id)
    }

    /// Get all stops of the trip folling the departure referenced
    fn stop_times(&self, &(trip_id, idx): &TripStopRef) -> &[StopTime] {
        &self
            .trips
            .get(&trip_id)
            .map(|trip| &trip.stop_times[idx..])
            .unwrap_or_default()
    }

    pub fn stops(&self) -> impl Iterator<Item = &Stop> {
        self.stops.values()
    }

    pub fn trips(&self) -> impl Iterator<Item = &Trip> {
        self.trips.values()
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum StopStereoType {
    // station is actually optional for stop or platform, but i think it is always present in vbbland
    StopOrPlatform {
        station: Option<StopId>,
        departures: BTreeMap<Time, Vec<TripStopRef>>,
    },
    Station {
        stops_or_platforms: Vec<StopId>,
    },
    EntranceExit {
        station: StopId,
    },
    // BoardingArea { stopOrPlatform: StopId },
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Stop {
    pub stop_id: StopId,
    pub stop_name: String,
    pub location: geo::Point<f64>,
    /// Type of the location
    pub stereotype: StopStereoType,
    pub transfers: Vec<Transfer>,
}

impl fmt::Debug for Stop {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} [{:?}{}]",
            self.stop_name,
            self.stop_id,
            if self.is_station() { "*" } else { "" }
        )
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

impl Stop {
    /// finds all trips leaving the stop within a time period, using the provided services, includes the stop time for that stop and all following stops
    pub fn departures(&self, period: Period) -> Vec<&TripStopRef> {
        match self.stereotype {
            StopStereoType::StopOrPlatform {
                station: _,
                ref departures,
            } => departures
                .range(period)
                .map(|(_time, trip_stop_refs)| trip_stop_refs)
                .flatten()
                .collect(),
            StopStereoType::Station {
                stops_or_platforms: _,
            } => vec![],
            StopStereoType::EntranceExit { station: _ } => vec![],
        }
    }

    /// Id of the parent station or own ID if this is a station
    pub fn station_id(&self) -> StopId {
        match self.stereotype {
            StopStereoType::StopOrPlatform {
                station,
                departures: _,
            } => station.unwrap_or(self.stop_id),
            StopStereoType::Station {
                stops_or_platforms: _,
            } => self.stop_id,
            StopStereoType::EntranceExit { station } => station,
        }
    }

    /// Id of the parent station or None if this is a station
    pub fn parent_station(&self) -> Option<StopId> {
        match self.stereotype {
            StopStereoType::StopOrPlatform {
                station,
                departures: _,
            } => station,
            StopStereoType::Station {
                stops_or_platforms: _,
            } => None,
            StopStereoType::EntranceExit { station } => Some(station),
        }
    }

    pub fn children(&self) -> impl Iterator<Item = &StopId> {
        match self.stereotype {
            StopStereoType::StopOrPlatform {
                station: _,
                departures: _,
            } => [].iter(),
            StopStereoType::Station {
                ref stops_or_platforms,
            } => stops_or_platforms.iter(),
            StopStereoType::EntranceExit { station: _ } => [].iter(),
        }
    }

    /// a top level stop
    pub fn is_station(&self) -> bool {
        match self.stereotype {
            StopStereoType::StopOrPlatform {
                station,
                departures: _,
            } => station.is_none(),
            StopStereoType::Station {
                stops_or_platforms: _,
            } => true,
            StopStereoType::EntranceExit { station: _ } => false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Route {
    /// Identifies a route.
    pub route_id: RouteId,
    pub route_short_name: String,
    pub route_type: RouteType,
    pub route_color: String,
}

impl PartialEq for Route {
    fn eq(&self, rhs: &Self) -> bool {
        self.route_id == rhs.route_id
    }
}

impl Eq for Route {}

impl PartialOrd for Route {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Route {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.route_id.cmp(&other.route_id)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Trip {
    /// Identifies a route.
    pub route: Route,
    /// Identifies a set of dates when service is available for one or more routes.
    pub service_id: ServiceId,
    /// Identifies a trip.
    pub trip_id: TripId,
    pub stop_times: Vec<StopTime>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StopTime {
    /// Arrival time at a specific stop for a specific trip on a route. If there are not separate times for arrival and departure at a stop, enter the same value for arrival_time and departure_time. For times occurring after midnight on the service day, enter the time as a value greater than 24:00:00 in HH:MM:SS local time for the day on which the trip schedule begins.
    /// Scheduled stops where the vehicle strictly adheres to the specified arrival and departure times are timepoints. If this stop is not a timepoint, it is recommended to provide an estimated or interpolated time. If this is not available, arrival_time can be left empty. Further, indicate that interpolated times are provided with timepoint=0. If interpolated times are indicated with timepoint=0, then time points must be indicated with timepoint=1. Provide arrival times for all stops that are time points. An arrival time must be specified for the first and the last stop in a trip.
    pub arrival_time: Time,
    /// Departure time from a specific stop for a specific trip on a route. For times occurring after midnight on the service day, enter the time as a value greater than 24:00:00 in HH:MM:SS local time for the day on which the trip schedule begins. If there are not separate times for arrival and departure at a stop, enter the same value for arrival_time and departure_time. See the arrival_time description for more details about using timepoints correctly.
    /// The departure_time field should specify time values whenever possible, including non-binding estimated or interpolated times between timepoints.
    pub departure_time: Time,
    /// Identifies the serviced stop. All stops serviced during a trip must have a record in stop_times.txt. Referenced locations must be stops, not stations or station entrances. A stop may be serviced multiple times in the same trip, and multiple trips and routes may service the same stop.
    pub stop_id: StopId,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Transfer {
    /// Identifies a stop or station where a connection between routes ends. If this field refers to a station, the transfer rule applies to all child stops.
    pub to_stop_id: StopId,
    // / Indicates the type of connection for the specified (from_stop_id, to_stop_id) pair. Valid options are:
    // transfer_type: TransferType,
    /// Amount of time, in seconds, that must be available to permit a transfer between routes at the specified stops. The min_transfer_time should be sufficient to permit a typical rider to move between the two stops, including buffer time to allow for schedule variance on each route.
    pub min_transfer_time: Option<Duration>,
}

pub struct FilterBuilder<'r> {
    existing_data: &'r GTFSData,
    new_data: GTFSData,
}

impl<'r> FilterBuilder<'r> {
    pub fn keep_stop(&mut self, stop: &Stop) {
        self.new_data
            .stops
            .entry(stop.stop_id)
            .or_insert_with(|| stop.clone());
    }

    pub fn keep_trip(&mut self, trip_id: TripId) {
        if !self.new_data.trips.contains_key(&trip_id) {
            self.new_data.trips.insert(
                trip_id,
                self.existing_data
                    .trips
                    .get(&trip_id)
                    .expect("trip to be in existing data")
                    .clone(),
            );
        }
    }

    pub fn build(self) -> GTFSData {
        // self.new_data.stops = self.existing_data.stops.clone();
        self.new_data
    }
}

pub struct Builder {
    data: GTFSData,
    stop_children: HashMap<StopId, Vec<StopId>>,
    routes: HashMap<RouteId, Route>,
    departure_count: u64,
}

impl Builder {
    pub fn add_station(&mut self, stop_id: StopId, stop_name: String, location: geo::Point<f64>) {
        self.data.stops.insert(
            stop_id,
            Stop {
                stop_id,
                stop_name,
                location,
                stereotype: StopStereoType::Station {
                    stops_or_platforms: Default::default(),
                },
                transfers: Default::default(),
            },
        );
    }

    pub fn add_stop_or_platform(
        &mut self,
        stop_id: StopId,
        stop_name: String,
        location: geo::Point<f64>,
        station: Option<StopId>,
    ) {
        self.data.stops.insert(
            stop_id,
            Stop {
                stop_id,
                stop_name,
                location,
                stereotype: StopStereoType::StopOrPlatform {
                    station,
                    departures: Default::default(),
                },
                transfers: Default::default(),
            },
        );
        if let Some(station) = station {
            self.stop_children.entry(station).or_default().push(stop_id);
        }
    }

    pub fn add_entrance_or_exit(
        &mut self,
        stop_id: StopId,
        stop_name: String,
        location: geo::Point<f64>,
        station: StopId,
    ) {
        self.data.stops.insert(
            stop_id,
            Stop {
                stop_id,
                stop_name,
                location,
                stereotype: StopStereoType::EntranceExit { station },
                transfers: Default::default(),
            },
        );
        self.stop_children.entry(station).or_default().push(stop_id);
    }

    pub fn add_transfer(
        &mut self,
        from_stop_id: StopId,
        to_stop_id: StopId,
        min_transfer_time: Option<Duration>,
    ) {
        let stop = self
            .data
            .stops
            .get_mut(&from_stop_id)
            .expect("from_stop for transfer to be loaded");
        stop.transfers.push(Transfer {
            to_stop_id,
            min_transfer_time,
        });
    }

    pub fn add_route(
        &mut self,
        route_id: RouteId,
        route_short_name: String,
        route_type: RouteType,
        route_color: String,
    ) {
        self.routes.insert(
            route_id,
            Route {
                /// Identifies a route.
                route_id,
                route_short_name,
                route_type,
                route_color: route_color.to_owned(),
            },
        );
    }

    pub fn add_trip(&mut self, trip_id: TripId, route_id: RouteId, service_id: ServiceId) {
        let route: &Route = self
            .routes
            .get(&route_id)
            .expect("trip's route to have been added");
        let route: Route = (*route).clone();
        self.data.trips.insert(
            trip_id,
            Trip {
                trip_id,
                route,
                service_id,
                stop_times: Default::default(),
            },
        );
    }

    pub fn add_trip_stop(
        &mut self,
        trip_id: TripId,
        arrival_time: Time,
        departure_time: Time,
        stop_id: StopId,
    ) {
        let trip: &mut Trip = self
            .data
            .trips
            .get_mut(&trip_id)
            .expect("stop time added to be of added trip");
        let stop_ref = (trip_id, trip.stop_times.len());
        trip.stop_times.push(StopTime {
            arrival_time,
            departure_time,
            stop_id,
        });
        let stop = self
            .data
            .stops
            .get_mut(&stop_id)
            .expect("stop time to be referencing added stop");
        match &mut stop.stereotype {
            StopStereoType::Station {
                stops_or_platforms: _,
            } => panic!("trip stops at station"),
            StopStereoType::EntranceExit { station: _ } => panic!("trip stops at station entrance"),
            StopStereoType::StopOrPlatform {
                station: _,
                ref mut departures,
            } => departures.entry(departure_time).or_default().push(stop_ref),
        };
        self.departure_count += 1;
    }

    pub fn build(mut self) -> GTFSData {
        for (station_id, children) in self.stop_children {
            let station = self
                .data
                .stops
                .get_mut(&station_id)
                .expect("parent station to exist");
            match &mut station.stereotype {
                StopStereoType::Station {
                    ref mut stops_or_platforms,
                } => *stops_or_platforms = children,
                StopStereoType::StopOrPlatform {
                    station: _,
                    departures: _,
                } => panic!(
                    "stop or platform {:?} indicated as a parent station of {:?}",
                    station, children
                ),
                StopStereoType::EntranceExit { station: _ } => panic!(
                    "entrance or exit {:?} indicated as a parent station of {:?}",
                    station, children
                ),
            }
        }

        eprintln!(
            "{} departures of {} trips, leaving from {} stops",
            self.departure_count,
            self.data.trips.len(),
            self.data.stops.len()
        );

        self.data
    }
}
