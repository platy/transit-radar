use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use super::search_data::*;

#[derive(Serialize, Deserialize)]
pub enum GTFSDataSync {
    Initial(u64, GTFSData),
    Increment {
        stops: HashMap<StopId, Stop>,
        trips: HashMap<TripId, Trip>,
        session_id: u64,
        update_number: u32,
    },
}

// todo check update numbers
impl GTFSDataSync {
    pub fn merge_data(self, data: &mut Option<GTFSData>) -> &GTFSData {
        match self {
            Self::Initial(_, new_data) => {
                *data = Some(new_data);
                data.as_ref().unwrap()
            }
            Self::Increment {
                trips,
                stops,
                update_number,
                session_id,
            } => {
                if let Some(existing_data) = data {
                    existing_data.trips.extend(trips);
                    existing_data.stops.extend(stops);
                    &*existing_data
                } else {
                    panic!("bad sync: retrieved increment with no data locally");
                }
            }
        }
    }

    pub fn session_id(&self) -> u64 {
        match self {
            Self::Initial(session_id, _) => *session_id,
            Self::Increment {
                trips,
                stops,
                update_number,
                session_id,
            } => *session_id,
        }
    }
}

pub struct GTFSDataSession {
    trips: HashSet<TripId>,
    stops: HashSet<StopId>,
    session_id: u64,
    update_number: u32,
}

impl From<u64> for GTFSDataSession {
    fn from(session_id: u64) -> GTFSDataSession {
        GTFSDataSession {
            trips: HashSet::new(),
            stops: HashSet::new(),
            session_id,
            update_number: 0,
        }
    }
}

impl GTFSDataSession {
    pub fn add_data(&mut self, data: GTFSData) -> GTFSDataSync {
        if self.update_number == 0 {
            self.trips = data.trips.keys().cloned().collect();
            self.stops = data.stops.keys().cloned().collect();
            self.update_number = 1;
            GTFSDataSync::Initial(self.session_id, data)
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
            GTFSDataSync::Increment {
                stops,
                trips,
                update_number: self.update_number,
                session_id: self.session_id,
            }
        }
    }
}
