use std::error::Error;
use std::process;
use std::fmt;
use std::collections::{HashSet, HashMap, LinkedList};
use std::iter::FromIterator;
use std::path::{Path, PathBuf};

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

    fn get_ubahn_route(&self, short_name: &str) -> Result<gtfs::Route, Box<dyn Error>> {
        let mut rdr = self.open_csv("routes.txt")?;
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

    fn get_sunday_services(&self) -> Result<HashSet<gtfs::ServiceId>, Box<dyn Error>> {
        let mut rdr = self.open_csv("calendar.txt")?;
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

    fn get_trips(&self, route: Route, service_ids: HashSet<gtfs::ServiceId>, direction: DirectionId) -> Result<Vec<Trip>, Box<dyn Error>> {
        let mut rdr = self.open_csv("trips.txt")?;
        let mut trips = Vec::new();
        for result in rdr.deserialize() {
            let record: gtfs::Trip = result?;
            if record.route_id == route.route_id && service_ids.contains(&record.service_id) && record.direction_id == direction  {
                trips.push(record);
            }
        }
        Ok(trips)
    }

    fn get_stop_times_on_trips(&self, trip_ids: HashSet<TripId>) -> Result<Vec<StopTime>, Box<dyn Error>> {
        let mut rdr = self.open_csv("stop_times.txt")?;
        let mut trips = Vec::new();
        for result in rdr.deserialize() {
            let record: gtfs::StopTime = result?;
            if trip_ids.contains(&record.trip_id) {
                trips.push(record);
            }
        }
        Ok(trips)
    }

    fn get_stops_map(&self) -> Result<HashMap<StopId, Stop>, Box<dyn Error>> {
        let mut rdr = self.open_csv("stops.txt")?;
        let mut stops = HashMap::new();
        for result in rdr.deserialize() {
            let record: gtfs::Stop = result?;
            stops.insert(record.stop_id.clone(), record);
        }
        Ok(stops)
    }

    fn merge_trip(combined_trip_ids: &mut LinkedList<(StopId, u16)>, mut trip_stop_ids: Vec<StopId>) { // change to linked list
        if combined_trip_ids.is_empty() {
            combined_trip_ids.extend(trip_stop_ids.into_iter().map(|stop_id| (stop_id, 1)));
        } else if let Some(offset) = combined_trip_ids.iter().position(|e| Some(&e.0) == trip_stop_ids.iter().next()) {
            // the new trip starts within the combined trip
            let intersection_length = std::cmp::min(combined_trip_ids.len() - offset, trip_stop_ids.len());
            let (intersection, right_extension) = {
                let r = trip_stop_ids.split_off(intersection_length);
                (trip_stop_ids, r)
            };
            // increment count of intersection
            for ((c, count), n) in combined_trip_ids.iter_mut().skip(offset).zip(intersection.iter()) {
                assert_eq!(c, n);
                *count += 1;
            }
            // append the last extension
            combined_trip_ids.append(&mut right_extension.into_iter().map(|stop_id| (stop_id, 1)).collect());
        } else if let Some(offset) = trip_stop_ids.iter().position(|e| Some(e) == combined_trip_ids.iter().next().map(|(id, _n)| id)) {
            // the combined trip starts within the new trip
            let intersection_length = std::cmp::min(combined_trip_ids.len(), trip_stop_ids.len() - offset);
            let (left_extension, intersection, right_extension) = {
                let mut i = trip_stop_ids.split_off(offset);
                let r = i.split_off(intersection_length);
                (trip_stop_ids, i, r)
            };
            println!("(left_extension, intersection, right_extension) : {:?}", (&left_extension, &intersection, &right_extension));
            // make sure the intersection matches
            for ((c, count), n) in combined_trip_ids.iter_mut().take(intersection_length).zip(intersection.iter()) {
                assert_eq!(c, n);
                *count += 1;
            }
            // add first extension on new. NOTE: can use prepend which is currently experimental
            for n in left_extension.into_iter().rev() {
                combined_trip_ids.push_front((n, 1));
            }
            // append the last extension
            combined_trip_ids.append(&mut right_extension.into_iter().map(|stop_id| (stop_id, 1)).collect());
        } else {
            panic!("trips don't match \n  {:?} \n  {:?}", combined_trip_ids, trip_stop_ids);
        }
    }

    fn example(&self) -> Result<(), Box<dyn Error>> {
        let sunday_services = self.get_sunday_services()?;
        println!("{} services", sunday_services.len());
        let route = self.get_ubahn_route("U8")?;
        let trips = self.get_trips(route, sunday_services, 0)?;
        println!("{} trips", trips.len());
        let trip_ids: HashSet<TripId> = HashSet::from_iter(trips.iter().map(|trip| trip.trip_id));
        let stops = self.get_stops_map()?;
        println!("{} stops", stops.len());

        let mut rdr = self.open_csv("stop_times.txt")?;
        struct CurrentTrip {
            _trip_id: TripId,
            origin_stop: StopTime,
            stop_ids: Vec<StopId>
        }
        let mut combined_stop_ids: LinkedList<(StopId, u16)> = LinkedList::new();
        let mut current_trip: Option<CurrentTrip> = None;
        for result in rdr.deserialize() {
            let record: gtfs::StopTime = result?;
            if trip_ids.contains(&record.trip_id) {
                let stop_id = record.stop_id[..record.stop_id.len() - 1].to_string();
                let stop_name = &stops.get(&record.stop_id).unwrap().stop_name;
                if record.stop_sequence == 0 {
                    if let Some(current_trip) = current_trip {
                        Self::merge_trip(&mut combined_stop_ids, current_trip.stop_ids);
                    }
                    println!("<<< {} >>> lookup more info?", record.trip_id);
                    println!("{:>2}m {}", 0, stop_name);
                    current_trip = Some(CurrentTrip {
                        _trip_id: record.trip_id,
                        stop_ids: [stop_id].as_ref().into(),
                        origin_stop: record,
                    });
                } else {
                    let current_trip = current_trip.as_mut().expect("not to be first record");
                    let origin_stop: &StopTime = &current_trip.origin_stop;
                    let wait = record.departure_time - origin_stop.departure_time;
                    current_trip.stop_ids.push(stop_id); // for the ubahn stations, there is a stop for each platform, the last digit is the platform so i remove it here to ignore
                    println!("{:>2}m {}", wait.mins(), stop_name);
                }
            }
        }
        println!("combined route");
        for (stop_id, count) in combined_stop_ids.iter_mut() {
            stop_id.push('1'); // put the platform id back on for the lookup
            let stop_name = &stops.get(stop_id).unwrap().stop_name;
            println!("{:>3} {}", count, stop_name);
        }
        Ok(())
    }
}

fn main() {
    if let Err(err) = GTFSSource::new(Path::new("./gtfs/")).example() {
        println!("error running example: {:?}", err);
        process::exit(1);
    }
}



#[test]
fn test_merge() {
    let mut c = LinkedList::new();
    let abc: Vec<_> = vec!["a", "b", "c"].iter().map(|&s| String::from(s)).collect();
    let bc: Vec<_> = vec!["b", "c"].iter().map(|&s| String::from(s)).collect();
    let bcd: Vec<_> = vec!["b", "c", "d"].iter().map(|&s| String::from(s)).collect();
    let abcd: Vec<_> = vec!["a", "b", "c", "d"].iter().map(|&s| String::from(s)).collect();
    let zab: Vec<_> = vec!["z", "a", "b"].iter().map(|&s| String::from(s)).collect();
    let zabcde: Vec<_> = vec!["z", "a", "b", "c", "d", "e"].iter().map(|&s| String::from(s)).collect();

    fn str_ref_c(c: &LinkedList<(StopId, u16)>) -> Vec<(&str, u16)> {
        c.iter().map(|(id, n)| (id.as_ref(), *n)).collect()
    }

    GTFSSource::merge_trip(&mut c, abc.clone());
    assert_eq!(str_ref_c(&c), vec![("a", 1), ("b", 1), ("c", 1)]);
    GTFSSource::merge_trip(&mut c, abc);
    assert_eq!(str_ref_c(&c), vec![("a", 2), ("b", 2), ("c", 2)]);
    GTFSSource::merge_trip(&mut c,  bc);
    assert_eq!(str_ref_c(&c), vec![("a", 2), ("b", 3), ("c", 3)]);
    GTFSSource::merge_trip(&mut c, bcd.clone());
    assert_eq!(str_ref_c(&c), vec![("a", 2), ("b", 4), ("c", 4), ("d", 1)]);
    GTFSSource::merge_trip(&mut c, zab);
    assert_eq!(str_ref_c(&c), vec![("z", 1), ("a", 3), ("b", 5), ("c", 4), ("d", 1)]);

    let mut c = LinkedList::new();
    GTFSSource::merge_trip(&mut c, abcd);
    assert_eq!(str_ref_c(&c), vec![("a", 1), ("b", 1), ("c", 1), ("d", 1)]);
    GTFSSource::merge_trip(&mut c, bcd);
    assert_eq!(str_ref_c(&c), vec![("a", 1), ("b", 2), ("c", 2), ("d", 2)]);
    GTFSSource::merge_trip(&mut c, zabcde);
    assert_eq!(str_ref_c(&c), vec![("z", 1), ("a", 2), ("b", 3), ("c", 3), ("d", 3), ("e", 1)]);
}

#[test]
#[should_panic]
fn test_merge_not_match() {
    let abc = vec!["a", "b", "c"].iter().map(|&s| String::from(s)).collect();
    let def = vec!["d", "e", "f"].iter().map(|&s| String::from(s)).collect();
    let mut c = LinkedList::new();
    GTFSSource::merge_trip(&mut c, abc);
    GTFSSource::merge_trip(&mut c, def);
}
