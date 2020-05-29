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
    session_id: u64,
    update_number: u64,
}

pub trait ClientSession {
    type Data;
    type Increment;

    fn new(session_id: u64) -> Self;
    // adds data to the clients session, producing the sync data that needs to be sent to the client and updating the session stat
    fn add_data(&mut self, data: Self::Data) -> SyncData<Self::Data, Self::Increment>;
    fn update_number(&self) -> u64;
}

impl ClientSession for GTFSDataSession {
    type Data = GTFSData;
    type Increment = GTFSSyncIncrement;

    fn new(session_id: u64) -> GTFSDataSession {
        eprintln!("New session {}", session_id);
        GTFSDataSession {
            trips: HashSet::new(),
            stops: HashSet::new(),
            session_id,
            update_number: 0,
        }
    }

    fn add_data(&mut self, data: GTFSData) -> SyncData<GTFSData, GTFSSyncIncrement> {
        if self.update_number == 0 {
            self.trips = data.trips.keys().cloned().collect();
            self.stops = data.stops.keys().cloned().collect();
            self.update_number = 1;
            SyncData::Initial {
                session_id: self.session_id,
                data,
                update_number: 1,
            }
        } else {
            let mut trips = data.trips;
            for id in self.trips.iter() {
                trips.remove(&id);
            }
            for &id in trips.keys() {
                self.trips.insert(id);
            }
            let mut stops = data.stops;
            for id in self.stops.iter() {
                stops.remove(&id);
            }
            for &id in stops.keys() {
                self.stops.insert(id);
            }
            self.update_number += 1;
            SyncData::Increment {
                increment: GTFSSyncIncrement { trips, stops },
                update_number: self.update_number,
                session_id: self.session_id,
            }
        }
    }

    fn update_number(&self) -> u64 {
        self.update_number
    }
}
