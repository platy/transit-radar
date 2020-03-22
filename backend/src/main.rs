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



struct SuperIter<'r, R: 'r + std::io::Read> {
    records: std::iter::Peekable<csv::DeserializeRecordsIter<'r, R, StopTime>>,
    trip_id: Option<TripId>,
}
impl <'r, R: 'r + std::io::Read> SuperIter<'r, R> {
    fn next<'s>(&'s mut self) -> Option<csv::Result<(TripId, Iter<'s, 'r, R>)>> {
        // skip any reecords with the existing trip id
        let mut next;
        loop {
            next = self.records.peek();
            if self.trip_id == next.and_then(|result| result.as_ref().ok()).map(|stop_time| stop_time.trip_id) {
                self.records.next(); // skip as its the old trip
            } else {
                break;
            }
        }
        // next is now either a new trip, an error or none
        if let Some(Ok(stop_time)) = next {
            let trip_id = stop_time.trip_id;
            self.trip_id = Some(trip_id);
            Some(Ok((stop_time.trip_id, Iter{records: &mut self.records, trip_id: trip_id})))
        } else if let Some(Err(_error)) = next {
            // if next is an error, consume it
            Some(Err(self.records.next().unwrap().unwrap_err()))
        } else {
            None
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

struct GTFSSource {
    dir_path: PathBuf,
}

impl GTFSSource {
    fn new(dir_path: &Path) -> GTFSSource {
        GTFSSource {
            dir_path: dir_path.to_owned(),
        }
    }

    fn open_csv(&self, filename: &str) -> Result<csv::Reader<std::fs::File>, csv::Error> {
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
        let mut combined_stop_ids: LinkedList<(StopId, Duration, u16)> = LinkedList::new();
        let mut rdr = self.open_csv("stop_times.txt")?;
        let mut iter = SuperIter {
            records: rdr.deserialize().peekable(),
            trip_id: None,
        };
        while let Some(Ok((trip_id, stop_times))) = iter.next() {
            if trip_ids.contains(&trip_id) {
                println!("<<< {} >>> lookup more info?", trip_id);
                let mut stop_ids = vec![];
                let mut previous_stop: Option<StopTime> = None;
                for record in stop_times {
                    let stop_time = record?;
                    let station = stations.get(&stop_time.stop_id).unwrap();
                    let stop_name = &station.stop_name;
                    let wait: Duration = previous_stop.map(|previous_stop| stop_time.departure_time - previous_stop.departure_time).unwrap_or_default();
                    stop_ids.push((station.stop_id.clone(), Some(wait))); // for the ubahn stations, there is a stop for each platform, the last digit is the platform so i remove it here to ignore
                    println!("{:>2}m {}", wait.mins(), stop_name);
                    previous_stop = Some(stop_time);
                }
                Self::merge_trip(&mut combined_stop_ids, stop_ids);
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
        let mut trips = vec![];

        let mut rdr = self.open_csv("stop_times.txt")?;
        let mut iter = SuperIter {
            records: rdr.deserialize().peekable(),
            trip_id: None,
        };
        while let Some(Ok((trip_id, stop_times))) = iter.next() {
            if available_trips.contains_key(&trip_id) {
                let mut on_trip: Option<LinkedList<(StopId, Duration)>> = None;
                for result in stop_times {
                    let stop_time = result?;
                    if let Some(on_trip) = on_trip.as_mut() {
                        let new_stop = (stop_time.stop_id, stop_time.arrival_time - time);
                        on_trip.push_back(new_stop);
                    } else if departure_stops.contains(&stop_time.stop_id)
                        && stop_time.departure_time.is_after(time) 
                        && stop_time.departure_time.is_before(time + Duration::minutes(30)) { // TODO should only include others with different destinations and possibly merge
                        // start trip
                        let new_stop = (stop_time.stop_id, stop_time.arrival_time - time);
                        on_trip = Some(LinkedList::from_iter(vec![new_stop]));
                    }
                }
                if let Some(on_trip) = on_trip {
                    trips.push((trip_id, on_trip));
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
