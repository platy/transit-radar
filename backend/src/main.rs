use std::error::Error;
use std::process;
use std::fmt;
use std::collections::{HashSet, HashMap, LinkedList};
use std::iter::FromIterator;
use std::path::{Path, PathBuf};

mod gtfs;
use gtfs::*;
use gtfs::gtfstime::{Duration, Time};

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

struct GTFSSource {
    dir_path: PathBuf,
}

impl GTFSSource {
    fn new(dir_path: &Path) -> GTFSSource {
        GTFSSource {
            dir_path: dir_path.to_owned(),
        }
    }

    fn open_csv(&self, filename: &str) -> Result<csv::Reader<std::fs::File>, Box<dyn Error>> {
        let path = self.dir_path.join(filename);
        println!("Opening {}", path.to_str().expect("path invalid"));
        let reader = csv::Reader::from_path(path)?;
        Ok(reader)
    }

    fn routes_by_id(&self) -> Result<HashMap<RouteId, Route>, Box<dyn Error>> {
        let mut rdr = self.open_csv("routes.txt")?;
        let mut routes = HashMap::new();
        for result in rdr.deserialize() {
            let record: gtfs::Route = result?;
            routes.insert(record.route_id.clone(), record);
        }
        Ok(routes)
    }

    fn get_ubahn_route(&self, short_name: &str) -> Result<gtfs::Route, Box<dyn Error>> {
        let mut rdr = self.open_csv("routes.txt")?;
        for result in rdr.deserialize() {
            let record: gtfs::Route = result?;
            if record.route_short_name == short_name && record.route_type == 400 {
                return Ok(record)
            }
        }
        Err(Box::new(MyError::NotFound))
    }

    fn get_sunday_services(&self) -> Result<HashSet<gtfs::ServiceId>, Box<dyn Error>> {
        let mut rdr = self.open_csv("calendar.txt")?;
        let mut services = HashSet::new();
        for result in rdr.deserialize() {
            let record: gtfs::Calendar = result?;
            if record.sunday == 1 { // should also filter the dat range
                services.insert(record.service_id);
            }
        }
        return Ok(services)
    }

    fn get_trips(&self, route_id: Option<RouteId>, service_ids: HashSet<gtfs::ServiceId>, direction: Option<DirectionId>) -> Result<Vec<Trip>, Box<dyn Error>> {
        let mut rdr = self.open_csv("trips.txt")?;
        let mut trips = Vec::new();
        for result in rdr.deserialize() {
            let record: gtfs::Trip = result?;
            if route_id.as_ref().map(|route_id| route_id == &record.route_id).unwrap_or(true)
                    && service_ids.contains(&record.service_id) 
                    && direction.map(|direction| direction == record.direction_id).unwrap_or(true) {
                trips.push(record);
            }
        }
        Ok(trips)
    }

    fn get_stops(&self) -> Result<Vec<Stop>, Box<dyn Error>> {
        let mut rdr = self.open_csv("stops.txt")?;
        let mut stops = Vec::new();
        for result in rdr.deserialize() {
            let record: gtfs::Stop = result?;
            stops.push(record);
        }
        Ok(stops)
    }

    fn stops_of_station(&self, station_id: StopId) -> Result<HashSet<StopId>, Box<dyn Error>> {
        let mut rdr = self.open_csv("stops.txt")?;
        let mut stops = Vec::new();
        for result in rdr.deserialize() {
            let record: gtfs::Stop = result?;
            if record.parent_station.as_ref() == Some(&station_id) {
                stops.push(record);
            }
        }
        Ok(stops.into_iter().map(|stop| stop.stop_id).collect())
    }

    fn stops_by_id(&self, stops: Vec<Stop>) -> HashMap<StopId, Stop> {
        let mut stops_by_id = HashMap::new();
        for stop in stops {
            stops_by_id.insert(stop.stop_id.clone(), stop);
        }
        stops_by_id
    }

    fn parent_stations_by_id(stops_by_id: &HashMap<StopId, Stop>) -> HashMap<&StopId, &Stop> {
        let mut stations_by_id = HashMap::new();
        for stop in stops_by_id.values() {
            if let Some(parent) = &stop.parent_station {
                let parent_station = &stops_by_id[parent];
                stations_by_id.insert(&stop.stop_id, parent_station);
            } else {
                stations_by_id.insert(&stop.stop_id, &stop);
            }
        }
        stations_by_id
    }

    fn merge_trip(combined_trip_ids: &mut LinkedList<(StopId, Duration, u16)>, mut trip_stop_ids: Vec<(StopId, Option<Duration>)>) { // change to linked list
        fn new_to_first_combined((id, duration): (StopId, Option<Duration>)) -> (StopId, Duration, u16) {
            if let Some(duration) = duration {
                (id, duration, 1)
            } else {
                (id, Duration::seconds(0), 0)
            }
        }
        if combined_trip_ids.is_empty() {
            combined_trip_ids.extend(trip_stop_ids.into_iter().map(new_to_first_combined));
        } else if let Some(offset) = combined_trip_ids.iter().position(|(i, _a, _c)| Some(i) == trip_stop_ids.iter().next().map(|(id, _d)| id)) {
            // the new trip starts within the combined trip
            let intersection_length = std::cmp::min(combined_trip_ids.len() - offset, trip_stop_ids.len());
            let (intersection, right_extension) = {
                let r = trip_stop_ids.split_off(intersection_length);
                (trip_stop_ids, r)
            };
            // increment count of intersection
            for ((c, time_acc, count), (n, time_to)) in combined_trip_ids.iter_mut().skip(offset).zip(intersection.iter()) {
                assert_eq!(c, n);
                if let Some(time_to) = time_to {
                    *count += 1;
                    *time_acc += *time_to;
                }
            }
            // append the last extension
            combined_trip_ids.append(&mut right_extension.into_iter().map(new_to_first_combined).collect());
        } else if let Some(offset) = trip_stop_ids.iter().position(|(i, _d)| Some(i) == combined_trip_ids.iter().next().map(|(id, _a, _n)| id)) {
            // the combined trip starts within the new trip
            let intersection_length = std::cmp::min(combined_trip_ids.len(), trip_stop_ids.len() - offset);
            let (left_extension, intersection, right_extension) = {
                let mut i = trip_stop_ids.split_off(offset);
                let r = i.split_off(intersection_length);
                (trip_stop_ids, i, r)
            };
            println!("(left_extension, intersection, right_extension) : {:?}", (&left_extension, &intersection, &right_extension));
            // make sure the intersection matches
            for ((c, time_acc, count), (n, time_to)) in combined_trip_ids.iter_mut().take(intersection_length).zip(intersection.iter()) {
                assert_eq!(c, n);
                if let Some(time_to) = time_to {
                    *count += 1;
                    *time_acc += *time_to;
                }
            }
            // add first extension on new. NOTE: could use prepend which is currently experimental
            for new in left_extension.into_iter().rev() {
                combined_trip_ids.push_front(new_to_first_combined(new));
            }
            // append the last extension
            combined_trip_ids.append(&mut right_extension.into_iter().map(new_to_first_combined).collect());
        } else {
            panic!("trips don't match \n  {:?} \n  {:?}", combined_trip_ids, trip_stop_ids);
        }
    }

    fn get_average_stop_times_on_trips(&self, trip_ids: &HashSet<TripId>, stations: &HashMap<&StopId, &Stop>) -> Result<LinkedList<(StopId, Duration, u16)>, Box<dyn Error>> {
        let mut rdr = self.open_csv("stop_times.txt")?;
        struct CurrentTrip {
            _trip_id: TripId,
            previous_stop: StopTime,
            stop_ids: Vec<(StopId, Option<Duration>)>
        }
        let mut combined_stop_ids: LinkedList<(StopId, Duration, u16)> = LinkedList::new();
        let mut current_trip: Option<CurrentTrip> = None;
        for result in rdr.deserialize() {
            let record: gtfs::StopTime = result?;
            if trip_ids.contains(&record.trip_id) {
                let station = stations.get(&record.stop_id).unwrap();
                let stop_name = &station.stop_name;
                if record.stop_sequence == 0 {
                    if let Some(current_trip) = current_trip {
                        Self::merge_trip(&mut combined_stop_ids, current_trip.stop_ids);
                    }
                    println!("<<< {} >>> lookup more info?", record.trip_id);
                    println!("{:>2}m {}", 0, stop_name);
                    current_trip = Some(CurrentTrip {
                        _trip_id: record.trip_id,
                        stop_ids: [(station.stop_id.clone(), None)].as_ref().into(),
                        previous_stop: record,
                    });
                } else {
                    let current_trip = current_trip.as_mut().expect("not to be first record");
                    let previous_stop: &StopTime = &current_trip.previous_stop;
                    let wait = record.departure_time - previous_stop.departure_time;
                    current_trip.stop_ids.push((station.stop_id.clone(), Some(wait))); // for the ubahn stations, there is a stop for each platform, the last digit is the platform so i remove it here to ignore
                    println!("{:>2}m {}", wait.mins(), stop_name);
                    current_trip.previous_stop = record;
                }
            }
        }
        Ok(combined_stop_ids)
    }

    fn example(&self) -> Result<(), Box<dyn Error>> {
        let sunday_services = self.get_sunday_services()?;
        println!("{} services", sunday_services.len());
        let route = self.get_ubahn_route("U8")?;
        let trips = self.get_trips(Some(route.route_id), sunday_services, Some(0))?;
        println!("{} trips", trips.len());
        let trip_ids: HashSet<TripId> = HashSet::from_iter(trips.iter().map(|trip| trip.trip_id));
        let stops_by_id = self.stops_by_id(self.get_stops()?);
        let stops = Self::parent_stations_by_id(&stops_by_id);
        println!("{} stops", stops.len());

        let combined_stop_ids = self.get_average_stop_times_on_trips(&trip_ids, &stops)?;
        println!("combined route");
        let mut wait_acc = Duration::seconds(0);
        for (stop_id, duration_acc, count) in combined_stop_ids.iter() {
            let stop_name = &stops.get(stop_id).unwrap().stop_name;
            let wait = if *count > 0 {
                *duration_acc / (*count).into()
            } else {
                Duration::seconds(0)
            };
            wait_acc += wait;
            println!("{:>3} {:>2}m {}", count, wait_acc.mins(), stop_name);
        }
        Ok(())
    }

    fn non_branching_travel_times_from(&self, departure_stops: &HashSet<StopId>, available_trips: &HashMap<TripId, Trip>, time: gtfs::gtfstime::Time) -> Result<Vec<(TripId, LinkedList<(StopId, Duration)>)>, Box<dyn Error>> {
        let mut rdr = self.open_csv("stop_times.txt")?;

        let mut trips = vec![];
        let mut on_trip: Option<(TripId, LinkedList<(StopId, Duration)>)> = None;
        for result in rdr.deserialize() {
            let record: gtfs::StopTime = result?;
            if on_trip.iter().any(|(trip_id, _stops)| *trip_id == record.trip_id) {
                let new_stop = (record.stop_id, record.arrival_time - time);
                on_trip.as_mut().unwrap().1.push_back(new_stop);
            } else {
                if let Some(old_trip) = on_trip {
                    // end trip
                    trips.push(old_trip);
                    on_trip = None;
                }
                if departure_stops.contains(&record.stop_id)
                    && record.departure_time.is_after(time) 
                    && record.departure_time.is_before(time + Duration::minutes(30)) // TODO should only include others with different destinations and posibly merge
                    && available_trips.contains_key(&record.trip_id) {
                    // start trip
                    let new_stop = (record.stop_id, record.arrival_time - time);
                    let new_trip = (record.trip_id, LinkedList::from_iter(vec![new_stop]));
                    on_trip = Some(new_trip);
                }
            }
        }
        Ok(trips)
    }

    fn example2(&self) -> Result<(), Box<dyn Error>> {
        // let stops = self.stops_by_id(self.get_stops()?);
        let sunday_services = self.get_sunday_services()?;
        println!("{} services", sunday_services.len());
        let available_trips = self.get_trips(None, sunday_services, None)?;
        let available_trips: HashMap<TripId, Trip> = available_trips.into_iter().map(|trip| (trip.trip_id.clone(), trip)).collect();

        let departure_stops = self.stops_of_station(900000007103)?;
        println!("Departure stops : {:?}", departure_stops);
        let trips = self.non_branching_travel_times_from(&departure_stops, &available_trips, Time::parse("09:00:00")?)?;
        let stops_by_id = self.stops_by_id(self.get_stops()?);
        let routes_by_id = self.routes_by_id()?;
        for (trip_id, stops) in trips.iter() {
            println!("Route {} Trip {}", routes_by_id[&available_trips[trip_id].route_id].route_short_name, trip_id);
            for (stop_id, duration) in stops.iter() {
                println!("  {:>2}m {}", duration.mins(), stops_by_id[stop_id].stop_name);
            }
        }
        println!("{} trips shown", trips.len());
        Ok(())
    }
}

fn main() {
    if let Err(err) = GTFSSource::new(Path::new("./gtfs/")).example2() {
        println!("error running example: {:?}", err);
        process::exit(1);
    }
}



#[test]
fn test_merge() {
    fn to_trip(vec: Vec<StopId>) -> Vec<(StopId, Option<Duration>)> {
    let two_secs = Some(Duration::seconds(2));
        vec.iter().map(|&s| (s, two_secs)).collect()
    }

    let mut c = LinkedList::new();
    let abc: Vec<_> = to_trip(vec![1, 2, 3]);
    let bc: Vec<_> = to_trip(vec![2, 3]);
    let bcd: Vec<_> = to_trip(vec![2, 3, 4]);
    let abcd: Vec<_> = to_trip(vec![1, 2, 3, 4]);
    let zab: Vec<_> = to_trip(vec![99, 1, 2]);
    let zabcde: Vec<_> = to_trip(vec![99, 1, 2, 3, 4, 5]);

    fn str_ref_c(c: &LinkedList<(StopId, Duration, u16)>) -> Vec<(StopId, i32, u16)> {
        c.iter().map(|(id, dur_acc, n)| (*id, dur_acc.secs(), *n)).collect()
    }

    GTFSSource::merge_trip(&mut c, abc.clone());
    assert_eq!(str_ref_c(&c), vec![(1, 2, 1), (2, 2, 1), (3, 2, 1)]);
    GTFSSource::merge_trip(&mut c, abc);
    assert_eq!(str_ref_c(&c), vec![(1, 4, 2), (2, 4, 2), (3, 4, 2)]);
    GTFSSource::merge_trip(&mut c,  bc);
    assert_eq!(str_ref_c(&c), vec![(1, 4, 2), (2, 6, 3), (3, 6, 3)]);
    GTFSSource::merge_trip(&mut c, bcd.clone());
    assert_eq!(str_ref_c(&c), vec![(1, 4, 2), (2, 8, 4), (3, 8, 4), (4, 2, 1)]);
    GTFSSource::merge_trip(&mut c, zab);
    assert_eq!(str_ref_c(&c), vec![(99, 2, 1), (1, 6, 3), (2, 10, 5), (3, 8, 4), (4, 2, 1)]);

    let mut c = LinkedList::new();
    GTFSSource::merge_trip(&mut c, abcd);
    assert_eq!(str_ref_c(&c), vec![(1, 2, 1), (2, 2, 1), (3, 2, 1), (4, 2, 1)]);
    GTFSSource::merge_trip(&mut c, bcd);
    assert_eq!(str_ref_c(&c), vec![(1, 2, 1), (2, 4, 2), (3, 4, 2), (4, 4, 2)]);
    GTFSSource::merge_trip(&mut c, zabcde);
    assert_eq!(str_ref_c(&c), vec![(99, 2, 1), (1, 4, 2), (2, 6, 3), (3, 6, 3), (4, 6, 3), (5, 2, 1)]);
}

#[test]
#[should_panic]
fn test_merge_not_match() {
    let two_secs = Some(Duration::seconds(2));

    let abc = vec![1, 2, 3].iter().map(|&s| (s, two_secs)).collect();
    let def = vec![4, 5, 6].iter().map(|&s| (s, two_secs)).collect();
    let mut c = LinkedList::new();
    GTFSSource::merge_trip(&mut c, abc);
    GTFSSource::merge_trip(&mut c, def);
}
