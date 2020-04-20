use std::error::Error;
use std::fmt;
use std::collections::{HashSet, HashMap};
use std::path::{Path, PathBuf};
use std::ops::Deref;
use crate::suggester::Suggester;

use radar_search::time::*;
use radar_search::search_data::*;
use crate::gtfs;

/// Refers to a specific stop of a specific trip (an arrival / departure)
pub type TripStopRef = (TripId, usize); // usize refers to the index of the stop in the trip, should probably instead use stop sequence

impl gtfs::Calendar {
    /// A Vec of all the days that this servcice runs on between start and end dates
    pub fn days(&self) -> Vec<Day> {
        let mut days = vec![];
        for (day, val) in [Day::Monday, Day::Tuesday, Day::Wednesday, Day::Thursday, Day::Friday, Day::Saturday, Day::Sunday].iter()
                     .zip([self.monday, self.tuesday, self.wednesday, self.thursday, self.friday, self.saturday, self.sunday].iter()) {
            if *val > 0 {
                days.push(*day);
            }
        }
        days
    }
}

pub fn load_data(gtfs_dir: &Path, day_filter: DayFilter) -> Result<GTFSData, Box<dyn Error>> {
    let source = &GTFSSource::new(gtfs_dir);

    let mut services_by_day: HashMap<_, HashSet<_>> = HashMap::new();
    let mut timetable_start_date = String::default();
    for result in source.get_calendar()? {
        let calendar: gtfs::Calendar = result?;
        for day in calendar.days() {
            services_by_day.entry(day).or_default().insert(calendar.service_id);
        }
        timetable_start_date = calendar.start_date;
    }

    let mut builder = GTFSData::builder(services_by_day.clone(), timetable_start_date);

    let mut rdr = source.open_csv("stops.txt")?;
    for result in rdr.deserialize() {
        match result {
            Ok(gtfs::Stop {
                stop_id,
                stop_name,
                stop_lat,
                stop_lon,
                location_type,
                parent_station,
            }) => {
                let location = geo::Point::new(stop_lat, stop_lon);
                match (location_type, parent_station) {
                    (1, None)                 => builder.add_station(stop_id, stop_name, location),
                    (0, parent_station)       => builder.add_stop_or_platform(stop_id, stop_name, location, parent_station),
                    (2, Some(parent_station)) => builder.add_entrance_or_exit(stop_id, stop_name, location, parent_station),
                    (1, Some(parent_station)) => panic!("station {:?} has parent {:?}", stop_id, parent_station),
                    (2, None)                 => panic!("entrance {:?} with no parent", stop_id),
                    (t, _)                    => panic!("{:?} is unknown location type {}", stop_id, t),
                };
            },
            Err(err) => 
                // /// One of VBB's StopIds has 'D_' in front of it, I don't know why. That stop's parent is the same number without the 'D_', it is on a couple of trips but - we just show a warning and skip it
                eprintln!("Error parsing stop - skipped : {}", err),
        }
    }

    for result in source.open_csv("transfers.txt")?.deserialize::<gtfs::Transfer>() {
        match result {
            Ok(transfer) =>
                builder.add_transfer(transfer.from_stop_id, transfer.to_stop_id, transfer.min_transfer_time.map(|d| Duration::seconds(d.to_secs()))),
            Err(err) => 
                eprintln!("Error parsing transfer : {}", err),
        }
    }
    
    let mut rdr = source.open_csv("routes.txt")?;
    for result in rdr.deserialize() {
        let route: gtfs::Route = result?;
        builder.add_route(route.route_id.into_inner(), route.route_short_name, route.route_type.into());
    }
    
    let services = match day_filter {
        DayFilter::All => None,
        DayFilter::Single(day) => Some(services_by_day.get(&day).unwrap().clone()),
    };
    let mut added_trips = HashSet::new();
    for result in source.get_trips(None, services)? {
        let trip: gtfs::Trip = result?;
        builder.add_trip(trip.trip_id, trip.route_id.into_inner(), trip.service_id);
        added_trips.insert(trip.trip_id);
    }
    
    let mut rdr = source.open_csv("stop_times.txt")?;
    for result in rdr.deserialize::<gtfs::StopTime>() {
        match result {
            Ok(stop_time) =>
                if added_trips.contains(&stop_time.trip_id) {
                    builder.add_trip_stop(stop_time.trip_id, stop_time.arrival_time, stop_time.departure_time, stop_time.stop_id);
                } else {
                    eprintln!("Stop time parsed for ignored trip {}", stop_time.trip_id)
                }
            Err(err) => 
                eprintln!("Error parsing stop time : {}", err),
        }
    }

    Ok(builder.build())
}


/// Get a station by exact name
/// # Issues
/// * This could be handled by Suggester
/// * This does a full scan
pub fn get_station_by_name<'r>(data: &'r GTFSData, exact_name: &str) -> Result<&'r Stop, SearchError> {
    let mut candidates = vec![];
    for stop in data.stops() {
        if stop.is_station() && stop.stop_name == exact_name {
            candidates.push(stop);
        }
    }
    if candidates.len() == 0 {
        Err(SearchError::NotFound(exact_name.to_owned()))
    } else if candidates.len() > 1 {
        panic!("ambiguous search");
        // Err(SearchError::Ambiguous(candidates.into_iter().cloned().collect()))
    } else {
        Ok(candidates[0])
    }
}

/// Build a word search suggester over station names
pub fn build_station_word_index(data: &GTFSData) -> Suggester<StopId> {
    let mut suggester = Suggester::new();

    for stop in data.stops() {
        if stop.is_station() {
            suggester.insert(&stop.stop_name, stop.stop_id);
        }
    }
    
    eprintln!("built station name index of {} words", suggester.num_words());
    suggester
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

  pub fn get_calendar(&self) -> Result<impl Iterator<Item = Result<gtfs::Calendar, csv::Error>>, csv::Error> {
    let rdr = self.open_csv("calendar.txt")?;
    Ok(rdr.into_deserialize())
  }

  pub fn get_trips(&self, route_id: Option<RouteId>, service_ids: Option<HashSet<ServiceId>>) -> Result<impl Iterator<Item = Result<gtfs::Trip, csv::Error>>, csv::Error> {
    let rdr = self.open_csv("trips.txt")?;
    let iter = rdr.into_deserialize().filter(move |result: &Result<gtfs::Trip, csv::Error>| {
        if let Ok(trip) = result {
            route_id.map(|route_id| route_id == trip.route_id.into_inner()).unwrap_or(true)
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

