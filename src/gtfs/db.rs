use crate::suggester::Suggester;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt;
use std::num::IntErrorKind;
use std::ops::Deref;
use std::path::{Path, PathBuf};

use crate::gtfs;
use csv::DeserializeErrorKind;
use radar_search::search_data::*;
use regex::Regex;

/// Refers to a specific stop of a specific trip (an arrival / departure)
pub type TripStopRef = (TripId, usize); // usize refers to the index of the stop in the trip, should probably instead use stop sequence

impl gtfs::Calendar {
    /// A Vec of all the days that this servcice runs on between start and end dates
    pub fn days(&self) -> Vec<Day> {
        let mut days = vec![];
        for (day, val) in [
            Day::Monday,
            Day::Tuesday,
            Day::Wednesday,
            Day::Thursday,
            Day::Friday,
            Day::Saturday,
            Day::Sunday,
        ]
        .iter()
        .zip(
            [
                self.monday,
                self.tuesday,
                self.wednesday,
                self.thursday,
                self.friday,
                self.saturday,
                self.sunday,
            ]
            .iter(),
        ) {
            if *val > 0 {
                days.push(*day);
            }
        }
        days
    }
}

fn color_for_type(route_type: RouteType) -> &'static str {
    match route_type {
        RouteType::SuburbanRailway => "lightgray",
        RouteType::UrbanRailway => "lightgray",
        RouteType::TramService => "lightgray",
        RouteType::Rail => "#e2001a",
        RouteType::RailwayService => "#e2001a",
        RouteType::Bus => "#a01c7d", // not sure if this is bus
        RouteType::BusService => "#a01c7d",
        RouteType::WaterTransportService => "#0099d6",
    }
}

pub fn load_data<S: std::hash::BuildHasher>(
    gtfs_dir: &Path,
    day_filter: DayFilter,
    route_colors: HashMap<String, String, S>,
) -> Result<GTFSData, Box<dyn Error>> {
    let source = &GTFSSource::new(gtfs_dir);

    let mut services_by_day: HashMap<_, HashSet<_>> = HashMap::new();
    let mut timetable_start_date = String::default();
    for result in source.get_calendar()? {
        let calendar: gtfs::Calendar = result?;
        for day in calendar.days() {
            services_by_day
                .entry(day)
                .or_default()
                .insert(calendar.service_id);
        }
        timetable_start_date = calendar.start_date;
    }

    let mut builder = GTFSData::builder(services_by_day.clone(), timetable_start_date);

    let mut interner = lasso::Rodeo::default();

    let mut count_stop_id_invalid_digit = 0;
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
                if location_type == 3 {
                    // generic node, for pathways, not used yet in transit radar
                    continue;
                }
                let stop_id = interner.get_or_intern(stop_id).into_inner();
                let parent_station =
                    parent_station.map(|stop_id| interner.get_or_intern(stop_id).into_inner());
                let short_stop_name = strip_stop_name(&stop_name);
                let location = geo::Point::new(stop_lat, stop_lon);
                match (location_type, parent_station) {
                    (1, None) => builder.add_station(stop_id, stop_name, short_stop_name, location),
                    (0, parent_station) => builder.add_stop_or_platform(
                        stop_id,
                        stop_name,
                        short_stop_name,
                        location,
                        parent_station,
                    ),
                    (2, Some(parent_station)) => builder.add_entrance_or_exit(
                        stop_id,
                        stop_name,
                        short_stop_name,
                        location,
                        parent_station,
                    ),
                    (1, Some(parent_station)) => {
                        panic!("station {:?} has parent {:?}", stop_id, parent_station)
                    }
                    (2, None) => panic!("entrance {:?} with no parent", stop_id),
                    (t, _) => panic!("{:?} is unknown location type {}", stop_id, t),
                };
            }
            Err(err) =>
            // /// One of VBB's StopIds has 'D_' in front of it, I don't know why. That stop's parent is the same number without the 'D_', it is on a couple of trips but - we just show a warning and skip it
            {
                if let csv::ErrorKind::Deserialize { pos: _, err } = err.kind() {
                    if err.field() == Some(0) {
                        if let DeserializeErrorKind::ParseInt(err) = err.kind() {
                            if IntErrorKind::InvalidDigit == *err.kind() {
                                count_stop_id_invalid_digit += 1;
                                continue;
                            }
                        }
                    }
                }
                eprintln!("Error parsing stop - skipped : {}", err)
            }
        }
    }
    log_invalid_digit_count_failures("stops", count_stop_id_invalid_digit);

    let mut count_stop_id_invalid_digit = 0;
    for result in source
        .open_csv("transfers.txt")?
        .deserialize::<gtfs::Transfer>()
    {
        match result {
            Ok(transfer) => builder.add_transfer(
                interner.get_or_intern(transfer.from_stop_id).into_inner(),
                interner.get_or_intern(transfer.to_stop_id).into_inner(),
                transfer.min_transfer_time,
            ),
            Err(err) => {
                if let csv::ErrorKind::Deserialize { pos: _, err } = err.kind() {
                    if err.field() == Some(0) {
                        if let DeserializeErrorKind::ParseInt(err) = err.kind() {
                            if IntErrorKind::InvalidDigit == *err.kind() {
                                count_stop_id_invalid_digit += 1;
                                continue;
                            }
                        }
                    }
                }
                eprintln!("Error parsing transfer : {}", err)
            }
        }
    }
    log_invalid_digit_count_failures("stops", count_stop_id_invalid_digit);

    use std::borrow::Cow;
    let mut rdr = source.open_csv("routes.txt")?;
    for result in rdr.deserialize() {
        let route: gtfs::Route = result?;
        let route_color: Cow<str> = route_colors
            .get(&route.route_short_name)
            .map(Into::into)
            .unwrap_or_else(|| color_for_type(route.route_type).into());
        builder.add_route(
            route.route_id.into_inner(),
            route.route_short_name,
            route.route_type,
            route_color.into_owned(),
        );
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

    let mut count_stop_id_invalid_digit = 0;
    let mut rdr = source.open_csv("stop_times.txt")?;
    for result in rdr.deserialize::<gtfs::StopTime>() {
        match result {
            Ok(stop_time) => {
                if added_trips.contains(&stop_time.trip_id) {
                    builder.add_trip_stop(
                        stop_time.trip_id,
                        stop_time.arrival_time,
                        stop_time.departure_time,
                        interner.get_or_intern(stop_time.stop_id).into_inner(),
                    );
                } else {
                    eprintln!("Stop time parsed for ignored trip {}", stop_time.trip_id)
                }
            }
            Err(err) => {
                if let csv::ErrorKind::Deserialize { pos: _, err } = err.kind() {
                    if err.field() == Some(3) {
                        if let DeserializeErrorKind::ParseInt(err) = err.kind() {
                            if IntErrorKind::InvalidDigit == *err.kind() {
                                count_stop_id_invalid_digit += 1;
                                continue;
                            }
                        }
                    }
                }
                eprintln!("Error parsing stop time : {}", err)
            }
        }
    }
    log_invalid_digit_count_failures("stop times", count_stop_id_invalid_digit);

    Ok(builder.build())
}

fn strip_stop_name(stop_name: &str) -> String {
    let pattern = Regex::new(r"Berlin, |S |S\+U |U | Bhf| \(Berlin\)| \[.*]").unwrap();
    pattern.replace_all(stop_name, "").into_owned()
}

#[test]
fn test_strip() {
    for (input, output) in &[
        ("Berlin, Birkholzer Weg/Straße 8", "Birkholzer Weg/Straße 8"),
        ("S Mahlsdorf (Berlin) [Tram Bus Treskowstr.]", "Mahlsdorf"),
        ("S Strausberg [Tram]", "Strausberg"),
        (
            "Dallgow-Döberitz, Finkenkruger Str.",
            "Dallgow-Döberitz, Finkenkruger Str.",
        ),
        ("S+U Alexanderplatz (Berlin) [U2]", "Alexanderplatz"),
        ("S+U Gesundbrunnen Bhf (Berlin)", "Gesundbrunnen"),
        (
            "Berlin, S+U Alexanderplatz Bhf/Memhardstr.",
            "Alexanderplatz/Memhardstr.",
        ),
    ] {
        assert_eq!(strip_stop_name(input), *output);
    }
}

/// Get a station by exact name
/// # Issues
/// * This could be handled by Suggester
/// * This does a full scan
pub fn get_station_by_name<'r>(
    data: &'r GTFSData,
    exact_name: &str,
) -> Result<&'r Stop, SearchError> {
    let mut candidates = vec![];
    for stop in data.stops() {
        if stop.is_station() && stop.full_stop_name == exact_name {
            candidates.push(stop);
        }
    }
    if candidates.is_empty() {
        Err(SearchError::NotFound(exact_name.to_owned()))
    } else if candidates.len() > 1 {
        panic!("ambiguous search");
    // Err(SearchError::Ambiguous(candidates.into_iter().cloned().collect()))
    } else {
        Ok(candidates[0])
    }
}

/// Build a word search suggester over station names
pub fn build_station_word_index(data: &GTFSData) -> Suggester<(StopId, usize)> {
    let mut suggester = Suggester::new();

    for stop in data.stops() {
        if stop.is_station() {
            suggester.insert(&stop.full_stop_name, (stop.stop_id, stop.importance(data)));
        }
    }

    eprintln!(
        "built station name index of {} words",
        suggester.num_words()
    );
    suggester
}

#[derive(Debug)]
pub enum SearchError {
    NotFound(String),
    Ambiguous(Vec<Stop>),
}

impl Error for SearchError {}

impl fmt::Display for SearchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SearchError::NotFound(search) => {
                write!(f, "Couldn't find stations for search term \"{}\"", search)
            }
            SearchError::Ambiguous(stops) => write!(
                f,
                "Found several stations or search term ({})",
                stops
                    .iter()
                    .map(|stop| stop.full_stop_name.clone())
                    .collect::<Vec<_>>()
                    .deref()
                    .join(", ")
            ),
        }
    }
}

pub fn load_colors(path: &Path) -> Result<HashMap<String, String>, csv::Error> {
    let mut colors = HashMap::new();
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(b';')
        .flexible(true)
        .from_path(path)?;
    let header = reader.headers().expect("headers expected in colors csv");
    eprintln!("{:?}", header);
    let route_name_idx = header
        .iter()
        .enumerate()
        .find(|(_, header)| *header == "Name")
        .expect("Header {Name}")
        .0;
    let colour_idx = header
        .iter()
        .enumerate()
        .find(|(_, header)| *header == "Hex")
        .expect("Header {Hex}")
        .0;
    for result in reader.into_records() {
        let br = result?;
        if let (Some(route_name), Some(colour)) = (
            br.get(route_name_idx).filter(|s| !s.is_empty()),
            br.get(colour_idx).filter(|s| !s.is_empty()),
        ) {
            colors.insert(route_name.to_owned(), colour.to_owned());
        }
    }
    Ok(colors)
}

pub struct GTFSSource {
    dir_path: PathBuf,
}

impl GTFSSource {
    pub fn new(dir_path: impl AsRef<Path>) -> GTFSSource {
        GTFSSource {
            dir_path: dir_path.as_ref().to_path_buf(),
        }
    }

    pub fn open_csv(&self, filename: &str) -> Result<csv::Reader<std::fs::File>, csv::Error> {
        let path = self.dir_path.join(filename);
        eprintln!("Opening {}", path.to_str().expect("path invalid"));
        let reader = csv::Reader::from_path(path)?;
        Ok(reader)
    }

    pub fn get_calendar(
        &self,
    ) -> Result<impl Iterator<Item = Result<gtfs::Calendar, csv::Error>>, csv::Error> {
        let rdr = self.open_csv("calendar.txt")?;
        Ok(rdr.into_deserialize())
    }

    pub fn get_trips(
        &self,
        route_id: Option<RouteId>,
        service_ids: Option<HashSet<ServiceId>>,
    ) -> Result<impl Iterator<Item = Result<gtfs::Trip, csv::Error>>, csv::Error> {
        let rdr = self.open_csv("trips.txt")?;
        let iter = rdr
            .into_deserialize()
            .filter(move |result: &Result<gtfs::Trip, csv::Error>| {
                if let Ok(trip) = result {
                    route_id
                        .map(|route_id| route_id == trip.route_id.into_inner())
                        .unwrap_or(true)
                        && service_ids
                            .as_ref()
                            .map(|service_ids| service_ids.contains(&trip.service_id))
                            .unwrap_or(true)
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

fn log_invalid_digit_count_failures(entity: &str, failure_count: u32) {
    if failure_count != 0 {
        eprintln!(
            "{failure_count} {entity} failed to parse due to an invalid digit in the stop id, this happens",
        );
    }
}
