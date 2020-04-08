use std::error::Error;
use std::fmt;
use std::collections::{HashSet, HashMap};
use std::path::{Path, PathBuf};
use crate::arena::{Arena, ArenaIndex, ArenaSliceIndex};
use std::ops::Deref;
use tst::TSTMap;

use crate::gtfs::*;
use crate::gtfs::gtfstime::{Period};

#[derive(Debug)]
pub enum SearchError {
    NotFound(String),
    Ambiguous(Vec<Stop>)
}

impl warp::reject::Reject for SearchError {}

impl Error for SearchError {}

impl fmt::Display for SearchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SearchError::NotFound(search) =>
                write!(f, "Couldn't find stations for search term \"{}\"", search),
            SearchError::Ambiguous(stops) =>
                write!(f, "Found several stations or search term ({})", stops.iter().map(|stop| stop.stop_name.clone()).collect::<Vec<_>>().deref().join(", ")),
        }
        
    }
}

struct SuperIter<'r, R: 'r + std::io::Read> {
    records: std::iter::Peekable<csv::DeserializeRecordsIter<'r, R, StopTime>>,
    trip_id: Option<TripId>,
}
impl <'r, R: 'r + std::io::Read> SuperIter<'r, R> {
    fn next<'s>(&'s mut self) -> Option<csv::Result<(TripId, Iter<'s, 'r, R>)>> {
        // skip any records with the existing trip id
        loop {
            match self.records.peek() {
                Some(Ok(stop_time)) => {
                    if self.trip_id == Some(stop_time.trip_id) {
                        self.records.next(); // skip as its the old trip
                    } else {
                        // Return a sub iterator, the internal next will be called next on that
                        let trip_id = stop_time.trip_id;
                        self.trip_id = Some(trip_id);
                        return Some(Ok((stop_time.trip_id, Iter{records: &mut self.records, trip_id: trip_id})))
                    }
                },
                // if next is an error, consume it
                Some(Err(_error)) => {
                    return Some(Err(self.records.next().unwrap().unwrap_err()))
                },
                None => return None,
            }
        }
    }
}

struct Iter<'s, 'r, R: 'r + std::io::Read> {
    records: &'s mut std::iter::Peekable<csv::DeserializeRecordsIter<'r, R, StopTime>>,
    trip_id: TripId,
}
impl <'s, 'r, R: 'r + std::io::Read> Iterator for Iter<'s, 'r, R> {
    type Item = csv::Result<StopTime>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(Ok(stop_time)) = self.records.peek() {
            if stop_time.trip_id != self.trip_id {
                return None
            }
        }
        self.records.next()
    }
}

pub struct GTFSSource {
    dir_path: PathBuf,
}

impl GTFSSource {
  pub fn new(dir_path: &Path) -> GTFSSource {
      GTFSSource {
          dir_path: dir_path.to_owned(),
      }
  }

  fn open_csv(&self, filename: &str) -> Result<csv::Reader<std::fs::File>, csv::Error> {
      let path = self.dir_path.join(filename);
      eprintln!("Opening {}", path.to_str().expect("path invalid"));
      let reader = csv::Reader::from_path(path)?;
      Ok(reader)
  }

  pub fn get_calendar(&self) -> Result<impl Iterator<Item = Result<Calendar, csv::Error>>, csv::Error> {
    let rdr = self.open_csv("calendar.txt")?;
    Ok(rdr.into_deserialize())
  }

  pub fn get_trips(&self, route_id: Option<RouteId>, service_ids: Option<HashSet<ServiceId>>, direction: Option<DirectionId>) -> Result<impl Iterator<Item = Result<Trip, csv::Error>>, csv::Error> {
    let rdr = self.open_csv("trips.txt")?;
    let iter = rdr.into_deserialize().filter(move |result: &Result<Trip, csv::Error>| {
        if let Ok(trip) = result {
            route_id.map(|route_id| route_id == trip.route_id).unwrap_or(true)
                && service_ids.as_ref().map(|service_ids| service_ids.contains(&trip.service_id)).unwrap_or(true)
                && direction.map(|direction| direction == trip.direction_id).unwrap_or(true)
        } else {
            false
        }
    });
    Ok(iter)
  }
}

pub struct GTFSData {
    stop_times_arena: Arena<StopTime>,
    stop_departures: HashMap<StopId, Vec<ArenaSliceIndex<StopTime>>>,
    transfers: HashMap<StopId, Vec<Transfer>>,
    stops_by_id: HashMap<StopId, Stop>,
    stops_by_parent_id: HashMap<StopId, Vec<StopId>>,
    services_by_day: HashMap<Day, HashSet<ServiceId>>,
    pub trips_by_id: HashMap<TripId, Trip>,
    routes_by_id: HashMap<RouteId, Route>,
    fake_stop: Stop,
    timetable_start_date: String,
}

#[derive(Copy, Clone)]
#[allow(dead_code)]
pub enum DayFilter {
    All,
    Single(Day),
}

impl std::fmt::Display for DayFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DayFilter::All => f.write_str("all"),
            DayFilter::Single(day) => day.fmt(f),
        }
    }
}

impl <'r> GTFSData {
    pub fn new() -> GTFSData {
        GTFSData {
            stop_times_arena: Arena::with_capacity(40000), // there are a lot and I don't want to risk keeping copying them
            stop_departures: HashMap::new(),
            transfers: HashMap::new(),
            stops_by_id: HashMap::new(),
            stops_by_parent_id: HashMap::new(),
            services_by_day: HashMap::new(),
            trips_by_id: HashMap::new(),
            routes_by_id: HashMap::new(),
            fake_stop: Stop::fake(),
            timetable_start_date: "".to_string(),
        }
    }

    pub fn timetable_start_date(&self) -> &str {
        &self.timetable_start_date
    }

    pub fn get_route_for_trip(&self, trip_id: &TripId) -> Option<&Route> {
        self.trips_by_id.get(trip_id).and_then(|trip| self.routes_by_id.get(&trip.route_id))
    }

    pub fn borrow_stop_departures(&self) -> &HashMap<StopId, Vec<ArenaSliceIndex<StopTime>>> {
      &self.stop_departures
    }

    pub fn load_stops_by_id(&mut self, source: &GTFSSource) -> Result<(), Box<dyn Error>> {
        let mut rdr = source.open_csv("stops.txt")?;
        for result in rdr.deserialize() {
            let stop: Stop = result?;
            self.stops_by_id.insert(stop.stop_id.clone(), stop);
        }
        self.stops_by_parent_id = Self::generate_stops_by_parent_id(&self.stops_by_id);
        Ok(())
    }

    pub fn load_calendar(&mut self, source: &GTFSSource) -> Result<(), Box<dyn Error>> {
        for result in source.get_calendar()? {
            let calendar: Calendar = result?;
            for day in calendar.days() {
                self.services_by_day.entry(day).or_default().insert(calendar.service_id);
            }
            self.timetable_start_date = calendar.start_date;
        }
        Ok(())
    }

    pub fn load_trips_by_id(&mut self, source: &GTFSSource, day_filter: DayFilter) -> Result<(), Box<dyn Error>> {
        let services = match day_filter {
            DayFilter::All => None,
            DayFilter::Single(day) => Some(self.services_by_day.get(&day).unwrap().clone()),
        };
        for result in source.get_trips(None, services, None)? {
            let trip: Trip = result?;
            self.trips_by_id.insert(trip.trip_id.clone(), trip);
        }
        Ok(())
    }

    pub fn load_routes_by_id(&mut self, source: &GTFSSource) -> Result<(), Box<dyn Error>> {
        let mut rdr = source.open_csv("routes.txt")?;
        for result in rdr.deserialize() {
            let route: Route = result?;
            self.routes_by_id.insert(route.route_id.clone(), route);
        }
        Ok(())
    }

    pub fn get_stop(&self, id: &StopId) -> Option<&Stop> {
        self.stops_by_id.get(id)
    }

    pub fn get_transfers(&self, stop_id: &StopId) -> Option<&Vec<Transfer>> {
      self.transfers.get(stop_id)
    }

    pub fn load_transfers_of_stop(&mut self, source: &GTFSSource) -> Result<(), Box<dyn Error>> {
        for result in source.open_csv("transfers.txt")?.deserialize() {
            let transfer: Transfer = result?;
            self.transfers.entry(transfer.from_stop_id).or_default().push(transfer);
        }
        Ok(())
    }

    pub fn get_station_by_name(&'r self, exact_name: &str) -> Result<&'r Stop, SearchError> {
        let mut candidates = vec![];
        for stop in self.stops_by_id.values() {
            if stop.parent_station.is_none() {
                if stop.stop_name == exact_name {
                    candidates.push(stop);
                }
            }
        }
        if candidates.len() == 0 {
            Err(SearchError::NotFound(exact_name.to_owned()))
        } else if candidates.len() > 1 {
            Err(SearchError::Ambiguous(candidates.into_iter().cloned().collect()))
        } else {
            Ok(candidates[0])
        }
    }

    pub fn services_of_day(&self, day: Day) -> HashSet<ServiceId> {
        self.services_by_day.get(&day).cloned().unwrap_or(HashSet::new())
    }

    pub fn departure_lookup(&'r mut self, period: Option<Period>, source: &GTFSSource) -> Result<(), Box<dyn Error>> {
        let mut rdr = source.open_csv("stop_times.txt")?;
        let mut iter = SuperIter {
            records: rdr.deserialize().peekable(),
            trip_id: None,
        };
        let mut count = 0;
        while let Some(result) = iter.next() {
            let (trip_id, stops) = result?;
            if self.trips_by_id.contains_key(&trip_id) {
                let stops = stops.skip_while(|result| 
                    if let (Ok(stop), Some(period)) = (result, period) {
                        !period.contains(stop.departure_time)
                    } else {
                        false
                    }
                );
                let stops = self.stop_times_arena.alloc_extend_result(stops)?;
                if stops.len() > 0 {
                    count += 1;
                }
                for (i, start_idx) in stops.iter().enumerate() {
                    let departures_from_stop = self.stop_departures.entry(self.stop_times_arena[start_idx].stop_id).or_default();
                    departures_from_stop.push(stops.sub(i..));
                }
            }
        }
        eprintln!("{} trips", count);
        eprintln!("{} departures allocated, leaving from {} stops", self.stop_times_arena.len(), self.stop_departures.len());

        Ok(())
    }

    fn generate_stops_by_parent_id(stops_by_id: &HashMap<StopId, Stop>) -> HashMap<StopId, Vec<StopId>> {
        let mut stops_by_parent_id: HashMap<StopId, Vec<StopId>> = HashMap::new();
        for stop in stops_by_id.values() {
            if let Some(parent) = stop.parent_station {
                stops_by_parent_id.entry(parent).or_default().push(stop.stop_id);
            }
        }
        stops_by_parent_id
    }

    pub fn stops_by_parent_id(&self, parent: &StopId) -> Vec<StopId> {
        self.stops_by_parent_id.get(parent).cloned().unwrap_or_default()
    }

    pub fn stops(&self, idx: ArenaSliceIndex<StopTime>) -> &[StopTime] {
        &self.stop_times_arena[idx]
    }

    pub fn stop(&self, id: ArenaIndex<StopTime>) -> &StopTime {
        &self.stop_times_arena[id]
    }

    pub fn fake_stop(&self) -> &Stop {
        &self.fake_stop
    }

    pub fn build_station_word_index<'t>(&self) -> Suggester<StopId> {
        let mut suggester = Suggester::new();

        let mut inserted_parents = HashSet::new();
        for stop_id in self.stop_departures.keys() {
            let stop = self.get_stop(stop_id).unwrap();
            if let Some(parent_station_id) = stop.parent_station {
                if inserted_parents.insert(parent_station_id) {
                    let stop = self.get_stop(&parent_station_id).unwrap();
                    suggester.insert(&stop.stop_name, stop.stop_id);
                }
            } else {
                suggester.insert(&stop.stop_name, stop.stop_id);
            }
        }
        
        eprintln!("built station name index of {} words", suggester.num_words());
        suggester
    }
}

pub struct Suggester<T> {
    map: TSTMap<HashSet<T>>,
}

impl<T: std::hash::Hash + Eq + Copy> Suggester<T> {
    fn new() -> Suggester<T> {
        Suggester {
            map: TSTMap::new(),
        }
    }

    fn insert(&mut self, key: &str, value: T) {
        for word in key.split_whitespace() {
            if word.len() > 3 {
                let v = self.map.entry(&word.to_lowercase()).or_insert(HashSet::new());
                v.insert(value);
            }
        }
    }

    fn num_words(&self) -> usize { self.map.len() }

    pub fn prefix_iter(&self, prefix: &str) -> impl Iterator<Item = (String, &HashSet<T>)> {
        self.map.prefix_iter(&prefix.to_lowercase())
    }

    pub fn search(&self, query: &str) -> impl IntoIterator<Item = T> {
        let query: Vec<_> = query.split_whitespace().collect();
        let mut results: HashSet<_> = self.prefix_iter(query[0]).map(|(_, s)| s).flatten().map(|i| *i).collect();
        for part in &query[1..] {
            let previous_results = results;
            results = self.prefix_iter(&part).map(|(_, s)| s).flatten().map(|i| *i).filter(|val| previous_results.contains(val)).collect();
        }
        results
    }
}
