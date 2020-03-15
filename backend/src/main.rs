use std::error::Error;
use std::process;
use std::fmt;
use std::collections::HashSet;
use std::iter::FromIterator;

mod gtfs;
use gtfs::*;

#[derive(Debug)]
enum MyError {
    NotFound,
}

impl fmt::Display for MyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MyError::NotFound => write!(f, "Something was not found")
        }
    }
}

impl Error for MyError {}

fn get_ubahn_route(short_name: &str) -> Result<gtfs::Route, Box<dyn Error>> {
    let mut rdr = csv::Reader::from_reader(std::fs::File::open("backend/gtfs/routes.txt")?);
    for result in rdr.deserialize() {
        // Notice that we need to provide a type hint for automatic
        // deserialization.
        let record: gtfs::Route = result?;
        if record.route_short_name == short_name && record.route_type == 400 {
            return Ok(record)
        }
    }
    Err(Box::new(MyError::NotFound))
}

fn get_sunday_services() -> Result<HashSet<gtfs::ServiceId>, Box<dyn Error>> {
    let mut rdr = csv::Reader::from_reader(std::fs::File::open("backend/gtfs/calendar.txt")?);
    let mut services = HashSet::new();
    for result in rdr.deserialize() {
        // Notice that we need to provide a type hint for automatic
        // deserialization.
        let record: gtfs::Calendar = result?;
        if record.sunday == 1 { // should also filter the dat range
            services.insert(record.service_id);
        }
    }
    return Ok(services)
}

fn get_trips(route: Route, service_ids: HashSet<gtfs::ServiceId>) -> Result<Vec<Trip>, Box<dyn Error>> {
    let mut rdr = csv::Reader::from_reader(std::fs::File::open("backend/gtfs/trips.txt")?);
    let mut trips = Vec::new();
    for result in rdr.deserialize() {
        // Notice that we need to provide a type hint for automatic
        // deserialization.
        let record: gtfs::Trip = result?;
        if record.route_id == route.route_id && service_ids.contains(&record.service_id) {
            trips.push(record);
        }
    }
    Ok(trips)
}

fn get_stop_times_on_trips(trip_ids: HashSet<TripId>) -> Result<Vec<StopTime>, Box<dyn Error>> {
    let mut rdr = csv::Reader::from_reader(std::fs::File::open("backend/gtfs/stop_times.txt")?);
    let mut trips = Vec::new();
    for result in rdr.deserialize() {
        let record: gtfs::StopTime = result?;
        if trip_ids.contains(&record.trip_id) {
            trips.push(record);
        }
    }
    Ok(trips)
}

fn example() -> Result<(), Box<dyn Error>> {
    let sunday_services = get_sunday_services()?;
    println!("{} services", sunday_services.len());
    let route = get_ubahn_route("U8")?;
    let trips = get_trips(route, sunday_services)?;
    println!("{} trips", trips.len());
    let trip_ids: HashSet<TripId> = HashSet::from_iter(trips.iter().map(|trip| trip.trip_id));

    let mut rdr = csv::Reader::from_reader(std::fs::File::open("backend/gtfs/stop_times.txt")?);
    for result in rdr.deserialize() {
        let record: gtfs::StopTime = result?;
        if trip_ids.contains(&record.trip_id) {
            println!("{:?}", record);
        }
    }
    Ok(())
}

fn main() {
    if let Err(err) = example() {
        println!("error running example: {:?}", err);
        process::exit(1);
    }
}
