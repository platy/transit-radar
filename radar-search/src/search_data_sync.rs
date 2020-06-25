use super::naive_sync::*;
use super::search_data::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Serialize, Deserialize)]
pub struct GTFSSyncIncrement {
    stops: HashMap<StopId, Stop>,
    trips: HashMap<TripId, Trip>,
}

impl std::ops::AddAssign<GTFSSyncIncrement> for GTFSData {
    fn add_assign(&mut self, other: GTFSSyncIncrement) {
        self.trips.extend(other.trips);
        self.stops.extend(other.stops);
    }
}

pub struct GTFSDataSession {
    trips: HashSet<TripId>,
    stops: HashSet<StopId>,
    session_id: i64,
    update_number: u64,
    last_origin: Option<StopId>,
}

pub trait ClientSession {
    type Data;
    type Increment;

    fn new(session_id: i64) -> Self;
    fn update_number(&self) -> u64;
}

impl ClientSession for GTFSDataSession {
    type Data = GTFSData;
    type Increment = GTFSSyncIncrement;

    fn new(session_id: i64) -> GTFSDataSession {
        eprintln!(
            "{} {}: New session",
            chrono::Utc::now().to_rfc3339(),
            session_id
        );
        GTFSDataSession {
            trips: HashSet::new(),
            stops: HashSet::new(),
            session_id,
            update_number: 0,
            last_origin: None,
        }
    }

    fn update_number(&self) -> u64 {
        self.update_number
    }
}

impl GTFSDataSession {
    // adds data to the clients session, producing the sync data that needs to be sent to the client and updating the session stat
    pub fn add_data(
        &mut self,
        required_data: RequiredData,
        data_source: &GTFSData,
    ) -> SyncData<GTFSData, GTFSSyncIncrement> {
        if self.is_new_session() {
            let trips = Self::get_trips(&required_data.trips, data_source);
            let stops = Self::get_stops(&required_data.stops, data_source);

            self.trips = required_data.trips;
            self.stops = required_data.stops;
            self.update_number = 1;

            SyncData::Initial {
                session_id: self.session_id,
                data: GTFSData {
                    services_by_day: required_data.services_by_day,
                    timetable_start_date: required_data.timetable_start_date,
                    stops,
                    trips,
                },
                update_number: self.update_number,
            }
        } else {
            let mut trips = required_data.trips;
            trips.retain(|trip_id| !self.trips.contains(trip_id));

            let mut stops = required_data.stops;
            stops.retain(|stop_id| !self.stops.contains(stop_id));

            self.trips.extend(&trips);
            self.stops.extend(&stops);
            self.update_number += 1;

            SyncData::Increment {
                increment: GTFSSyncIncrement {
                    trips: Self::get_trips(&trips, data_source),
                    stops: Self::get_stops(&stops, data_source),
                },
                update_number: self.update_number,
                session_id: self.session_id,
            }
        }
    }

    fn get_trips(trip_ids: &HashSet<TripId>, data_source: &GTFSData) -> HashMap<TripId, Trip> {
        trip_ids
            .iter()
            .map(|&trip_id| (trip_id, data_source.trips.get(&trip_id).unwrap().clone()))
            .collect()
    }

    fn get_stops(stop_ids: &HashSet<StopId>, data_source: &GTFSData) -> HashMap<StopId, Stop> {
        stop_ids
            .iter()
            .map(|&stop_id| (stop_id, data_source.get_stop(stop_id).cloned().unwrap()))
            .collect()
    }

    pub fn record_search(&mut self, stop: &Stop) {
        if self.last_origin != Some(stop.stop_id) {
            self.last_origin = Some(stop.stop_id);
            eprintln!(
                r#"{} {}: New search "{}""#,
                chrono::Utc::now().to_rfc3339(),
                self.session_id,
                stop.stop_name
            );
        }
    }

    fn is_new_session(&self) -> bool {
        self.update_number == 0
    }
}
