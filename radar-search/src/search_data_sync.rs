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
        eprintln!("{} {}: New session", chrono::Utc::now().to_rfc3339(), session_id);
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
        data: RequiredData,
        data_source: &GTFSData,
    ) -> SyncData<GTFSData, GTFSSyncIncrement> {
        let to_send_stops: HashMap<StopId, Stop> = data
            .stops
            .into_iter()
            .map(|stop_id| (stop_id, data_source.get_stop(&stop_id).cloned().unwrap()))
            .collect();

        if self.update_number == 0 {
            self.trips = data.trips.iter().copied().collect();
            self.stops = to_send_stops.keys().copied().collect();
            self.update_number = 1;
            SyncData::Initial {
                session_id: self.session_id,
                data: GTFSData {
                    services_by_day: data.services_by_day,
                    timetable_start_date: data.timetable_start_date,
                    stops: to_send_stops,
                    trips: data
                        .trips
                        .into_iter()
                        .map(|trip_id| (trip_id, data_source.trips.get(&trip_id).unwrap().clone()))
                        .collect(),
                },
                update_number: 1,
            }
        } else {
            let mut trips = data.trips;
            trips.retain(|trip_id| !self.trips.contains(trip_id));
            self.trips.extend(&trips);

            self.stops.extend(to_send_stops.keys());
            self.update_number += 1;
            SyncData::Increment {
                increment: GTFSSyncIncrement {
                    trips: trips
                        .into_iter()
                        .map(|trip_id| (trip_id, data_source.trips.get(&trip_id).unwrap().clone()))
                        .collect(),
                    stops: to_send_stops,
                },
                update_number: self.update_number,
                session_id: self.session_id,
            }
        }
    }

    pub fn record_search(&mut self, stop: &Stop) {
        if self.last_origin != Some(stop.stop_id) {
            self.last_origin = Some(stop.stop_id);
            eprintln!(r#"{} {}: New search "{}""#, chrono::Utc::now().to_rfc3339(), self.session_id, stop.stop_name);
        }
    }
}
