use std::error::Error;
use std::fmt;
use std::collections::{HashSet, HashMap};
use std::path::{Path, PathBuf};
use std::ops::Deref;
use crate::suggester::Suggester;

use crate::gtfs::*;
use crate::gtfs::time::{Period};

/// Refers to a specific stop of a specific trip
pub type TripStopRef = (TripId, usize); // usize refers to the index of the stop in the trip, should probably instead use stop sequence

/// Parsed and indexed GTFS data
pub struct GTFSData {
    stops_by_id: HashMap<StopId, Stop>,
    stops_by_parent_id: HashMap<StopId, Vec<StopId>>,
    stop_departures: HashMap<StopId, Vec<TripStopRef>>,
    transfers: HashMap<StopId, Vec<Transfer>>,
    services_by_day: HashMap<Day, HashSet<ServiceId>>,
    trip_stop_times: HashMap<TripId, Vec<StopTime>>,
    trips_by_id: HashMap<TripId, Trip>,
    routes_by_id: HashMap<RouteId, Route>,
    timetable_start_date: String,
}

impl <'r> GTFSData {
    pub fn load_data(gtfs_dir: &Path, day_filter: DayFilter, time_period: Option<Period>) -> Result<GTFSData, Box<dyn Error>> {
        let source = &GTFSSource::new(gtfs_dir);
    
        let mut data = GTFSData {
            trip_stop_times: HashMap::new(),
            stop_departures: HashMap::new(),
            transfers: HashMap::new(),
            stops_by_id: HashMap::new(),
            stops_by_parent_id: HashMap::new(),
            services_by_day: HashMap::new(),
            trips_by_id: HashMap::new(),
            routes_by_id: HashMap::new(),
            timetable_start_date: "".to_string(),
        };
        data.load_calendar(source)?;
        data.load_transfers(source)?;
        data.load_stops(source)?;
        data.load_trips(source, day_filter)?;
        data.load_routes(source)?;
        data.load_departures(time_period, &source)?;
        Ok(data)
    }

    fn load_calendar(&mut self, source: &GTFSSource) -> Result<(), Box<dyn Error>> {
        for result in source.get_calendar()? {
            let calendar: Calendar = result?;
            for day in calendar.days() {
                self.services_by_day.entry(day).or_default().insert(calendar.service_id);
            }
            self.timetable_start_date = calendar.start_date;
        }
        Ok(())
    }

    fn load_transfers(&mut self, source: &GTFSSource) -> Result<(), Box<dyn Error>> {
        for result in source.open_csv("transfers.txt")?.deserialize() {
            let transfer: Transfer = result?;
            self.transfers.entry(transfer.from_stop_id).or_default().push(transfer);
        }
        Ok(())
    }

    fn load_stops(&mut self, source: &GTFSSource) -> Result<(), Box<dyn Error>> {
        let mut rdr = source.open_csv("stops.txt")?;
        for result in rdr.deserialize() {
            let stop: Stop = result?;
            self.stops_by_id.insert(stop.stop_id.clone(), stop);
        }
        for stop in self.stops_by_id.values() {
            if let Some(parent) = stop.parent_station {
                self.stops_by_parent_id.entry(parent).or_default().push(stop.stop_id);
            }
        }
        Ok(())
    }

    fn load_trips(&mut self, source: &GTFSSource, day_filter: DayFilter) -> Result<(), Box<dyn Error>> {
        let services = match day_filter {
            DayFilter::All => None,
            DayFilter::Single(day) => Some(self.services_by_day.get(&day).unwrap().clone()),
        };
        for result in source.get_trips(None, services)? {
            let trip: Trip = result?;
            self.trips_by_id.insert(trip.trip_id.clone(), trip);
        }
        Ok(())
    }

    fn load_routes(&mut self, source: &GTFSSource) -> Result<(), Box<dyn Error>> {
        let mut rdr = source.open_csv("routes.txt")?;
        for result in rdr.deserialize() {
            let route: Route = result?;
            self.routes_by_id.insert(route.route_id.clone(), route);
        }
        Ok(())
    }

    fn load_departures(&'r mut self, period: Option<Period>, source: &GTFSSource) -> Result<(), Box<dyn Error>> {
        let mut rdr = source.open_csv("stop_times.txt")?;
        let mut iter = SuperIter {
            records: rdr.deserialize().peekable(),
            trip_id: None,
        };
        let mut departure_count = 0;
        while let Some(result) = iter.next() {
            let (trip_id, stops) = result?;
            if self.trips_by_id.contains_key(&trip_id) {
                let stops: Result<Vec<StopTime>, _> = stops.skip_while(|result| 
                    if let (Ok(stop), Some(period)) = (result, period) {
                        !period.contains(stop.departure_time)
                    } else {
                        false
                    }
                ).collect();
                let stops = stops?;
                departure_count += stops.len();
                if stops.len() > 0 {
                    for (i, stop_time) in stops.iter().enumerate() {
                        let departures_from_stop = self.stop_departures.entry(stop_time.stop_id).or_default();
                        departures_from_stop.push((trip_id, i));
                    }
                    self.trip_stop_times.insert(trip_id, stops);
                }
            }
        }
        eprintln!("{} departures of {} trips allocated, leaving from {} stops", departure_count, self.trip_stop_times.len(), self.stop_departures.len());

        Ok(())
    }

    /// Start date of the timetable based upon the calendar records
    pub fn timetable_start_date(&self) -> &str {
        &self.timetable_start_date
    }

    /// Get the route that the specified trip is a part of
    pub fn get_route_for_trip(&self, trip_id: &TripId) -> &Route {
        self.trips_by_id.get(trip_id).and_then(|trip| self.routes_by_id.get(&trip.route_id)).expect("To have route entry for trip")
    }

    pub fn get_stop(&self, id: &StopId) -> Option<&Stop> {
        self.stops_by_id.get(id)
    }

    /// finds all trips leaving a stop within a time period, using the provided services, includes the stop time for that stop and all following stops
    pub fn trips_from(&self, stop: StopId, services: &HashSet<ServiceId>, period: Period) -> Vec<&[StopTime]> {
      let departures = self.get_departures_from(stop);
      departures.iter().filter(move |&stop_ref: &&TripStopRef| {
        // this is a slow lookup in a critical code section, if departure_time was part of the Ref this wouldn't be necessary
        let stop_time = self.stop_time(stop_ref);
        period.contains(stop_time.departure_time) && services.contains(&self.trips_by_id.get(&stop_time.trip_id).unwrap().service_id)
      }).map(|stop_ref| self.stop_times(&stop_ref)).collect()
    }

    /// Get all the transfers from a stop
    pub fn transfers_from(&self, stop_id: &StopId) -> &[Transfer] {
        self.transfers.get(stop_id).map(|vec| &vec[..]).unwrap_or_default()
    }

    /// Get a station by exact name
    /// # Issues
    /// * This could be handled by Suggester
    /// * This does a full scan
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

    /// Get all the services which run on a particular day of the week
    pub fn services_of_day(&self, day: Day) -> HashSet<ServiceId> {
        self.services_by_day.get(&day).cloned().unwrap_or(HashSet::new())
    }

    /// Get all child stops of a parent station
    pub fn stops_by_parent_id(&self, parent: &StopId) -> &[StopId] {
        self.stops_by_parent_id.get(parent).map(|vec| &vec[..]).unwrap_or_default()
    }

    /// Get all the ref for all departing trip stops from a stop
    fn get_departures_from(&self, stop_id: StopId) -> &[TripStopRef] {
        &self.stop_departures.get(&stop_id).map(|v| &v[..]).unwrap_or_default()
    }

    /// Get all stops of the trip folling the departure referenced
    fn stop_times(&self, &(trip_id, idx): &TripStopRef) -> &[StopTime] {
        &self.trip_stop_times.get(&trip_id).map(|stop_times| &stop_times[idx..]).unwrap_or_default()
    }

    /// get the initial stop time of the trip departure referenced
    fn stop_time(&self, &(trip_id, idx): &TripStopRef) -> &StopTime {
        &self.trip_stop_times.get(&trip_id).map(|stop_times| &stop_times[idx]).expect("Stop with this Ref")
    }

    /// Build a word search suggester over station names
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

  pub fn open_csv(&self, filename: &str) -> Result<csv::Reader<std::fs::File>, csv::Error> {
      let path = self.dir_path.join(filename);
      eprintln!("Opening {}", path.to_str().expect("path invalid"));
      let reader = csv::Reader::from_path(path)?;
      Ok(reader)
  }

  pub fn get_calendar(&self) -> Result<impl Iterator<Item = Result<Calendar, csv::Error>>, csv::Error> {
    let rdr = self.open_csv("calendar.txt")?;
    Ok(rdr.into_deserialize())
  }

  pub fn get_trips(&self, route_id: Option<RouteId>, service_ids: Option<HashSet<ServiceId>>) -> Result<impl Iterator<Item = Result<Trip, csv::Error>>, csv::Error> {
    let rdr = self.open_csv("trips.txt")?;
    let iter = rdr.into_deserialize().filter(move |result: &Result<Trip, csv::Error>| {
        if let Ok(trip) = result {
            route_id.map(|route_id| route_id == trip.route_id).unwrap_or(true)
                && service_ids.as_ref().map(|service_ids| service_ids.contains(&trip.service_id)).unwrap_or(true)
        } else {
            false
        }
    });
    Ok(iter)
  }
}

#[derive(Copy, Clone)]
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

