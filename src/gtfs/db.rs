use std::error::Error;
use std::fmt;
use std::collections::{HashSet, HashMap};
use std::path::{Path, PathBuf};
use std::cell::{RefCell, BorrowError, Ref};
use typed_arena::Arena;
use serde::{Serialize, Serializer, Deserialize, Deserializer, de::{Visitor, SeqAccess}, de};
use std::ops::Range;
use std::marker::PhantomData;

use crate::gtfs::*;
use crate::gtfs::gtfstime::{Period};


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

  pub fn load_cache(&self, period: Period) -> Result<Option<GTFSData>, Box<dyn Error>> {
    let path = self.dir_path.join(format!("cache-{}", period));
    if path.is_file() {
      let file = std::fs::File::open(path)?;
      let data = rmp_serde::decode::from_read(file)?;
      Ok(Some(data))
    } else {
      Ok(None)
    }
  }

  pub fn write_cache(&self, period: Period, data: &GTFSData) -> Result<(), Box<dyn Error>> {
    let path = self.dir_path.join(format!("cache-{}", period));
    let mut file = std::fs::File::create(path)?;
    rmp_serde::encode::write(&mut file, data)?;
    Ok(())
  }

  pub fn get_sunday_services(&self) -> Result<HashSet<ServiceId>, Box<dyn Error>> {
      let mut rdr = self.open_csv("calendar.txt")?;
      let mut services = HashSet::new();
      for result in rdr.deserialize() {
          let record: Calendar = result?;
          if record.sunday == 1 { // should also filter the dat range
              services.insert(record.service_id);
          }
      }
      return Ok(services)
  }

  pub fn get_trips(&self, route_id: Option<RouteId>, service_ids: HashSet<ServiceId>, direction: Option<DirectionId>) -> Result<Vec<Trip>, Box<dyn Error>> {
      let mut rdr = self.open_csv("trips.txt")?;
      let mut trips = Vec::new();
      for result in rdr.deserialize() {
          let record: Trip = result?;
          if route_id.as_ref().map(|route_id| route_id == &record.route_id).unwrap_or(true)
                  && service_ids.contains(&record.service_id) 
                  && direction.map(|direction| direction == record.direction_id).unwrap_or(true) {
              trips.push(record);
          }
      }
      Ok(trips)
  }
}

pub struct GTFSData<'r> {
    stop_times_arena: Arena<StopTime>,
    stop_departures: RefCell<HashMap<StopId, Vec<&'r[StopTime]>>>,
    transfers: HashMap<StopId, Vec<Transfer>>,
    stops_by_id: HashMap<StopId, Stop>,
    trips_by_id: HashMap<TripId, Trip>,
    routes_by_id: HashMap<RouteId, Route>,
}

/// only supports the struct being serialised as a sequence
impl<'de, 'r> Deserialize<'de> for GTFSData<'r> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct GTFSDataVisitor<'r> {
          marker: PhantomData<fn() -> GTFSData<'r>>,
        }

        impl<'de, 'r> Visitor<'de> for GTFSDataVisitor<'r> {
            type Value = GTFSData<'r>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct GTFSData")
            }

            /// serialisation is
            /// arena
            /// stop_departures_count: u32
            /// [(stop_id: StopId, Vec<Range<u32>); stop_departures_count]
            /// transfers
            /// stops_by_id
            fn visit_seq<V>(self, mut seq: V) -> Result<GTFSData<'r>, V::Error>
            where
                V: SeqAccess<'de>,
            {
                let stop_times_arena: Arena<StopTime> = seq.next_element()?.ok_or_else(|| de::Error::invalid_length(0, &self))?;
                eprintln!("read {} of arena", stop_times_arena.len());
                // i should be able to make some Visitor for this and perhaps extract a trait
                let stop_departures_count: usize = seq.next_element::<u32>()?.ok_or_else(|| de::Error::invalid_length(0, &self))? as usize;
                eprintln!("reading {} of departures", stop_departures_count);
                let mut stop_departures: HashMap<StopId, Vec<&'r[StopTime]>> = HashMap::with_capacity(stop_departures_count);
                for _i in 0..stop_departures_count {
                    let (stop_id, trips): (StopId, Vec<Range<u32>>) = seq.next_element()?.ok_or_else(|| de::Error::invalid_length(0, &self))?;
                    let trips: Vec<Range<usize>> = trips.iter().map(|range| range.start as usize..range.end as usize).collect();
                    stop_departures.insert(stop_id, trips.iter().map(|range| stop_times_arena.id_to_slice(range.clone())).collect());
                }
                eprintln!("read {} of departures", stop_departures_count);

                let transfers: HashMap<_,_> = seq.next_element()?.ok_or_else(|| de::Error::invalid_length(0, &self))?;
                eprintln!("read {} of transfers", transfers.len());
                let stops_by_id: HashMap<_,_> = seq.next_element()?.ok_or_else(|| de::Error::invalid_length(0, &self))?;
                eprintln!("read {} of stops_by_id", stops_by_id.len());
                let trips_by_id: HashMap<_,_> = seq.next_element()?.ok_or_else(|| de::Error::invalid_length(0, &self))?;
                eprintln!("read {} of trips_by_id", trips_by_id.len());
                let routes_by_id: HashMap<_,_> = seq.next_element()?.ok_or_else(|| de::Error::invalid_length(0, &self))?;
                eprintln!("read {} of routes_by_id", routes_by_id.len());
                Ok(GTFSData {
                    stop_times_arena,
                    stop_departures: RefCell::new(stop_departures),
                    transfers,
                    stops_by_id,
                    trips_by_id,
                    routes_by_id,
                })
            }
        }

        deserializer.deserialize_seq(GTFSDataVisitor { marker: PhantomData })
    }
}

use serde::ser::{SerializeSeq};


/// serialisation is
/// arena
/// stop_departures_count: u32
/// [(stop_id: StopId, Vec<Range<u32>); stop_departures_count]
/// transfers
/// stops_by_id
impl <'r> Serialize for GTFSData<'r> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let stop_departures = self.stop_departures.borrow();
        let mut seq = serializer.serialize_seq(Some(stop_departures.len() + 6))?; // this is stupid
        seq.serialize_element(&self.stop_times_arena)?;
        eprintln!("written {} of arena", self.stop_times_arena.len());
        seq.serialize_element(&stop_departures.len())?;
        eprintln!("writing {} of departures", stop_departures.len());
        for (stop_id, trips) in stop_departures.iter() {
            let trips: Vec<Range<u32>> = trips.iter().map(|slice| self.stop_times_arena.slice_to_id(slice)).map(|slice| (slice.start as u32)..(slice.end as u32)).collect();
            seq.serialize_element(&(stop_id, trips))?;
        }
        eprintln!("written {} of departures", stop_departures.len());

        seq.serialize_element(&self.transfers)?;
        seq.serialize_element(&self.stops_by_id)?;
        seq.serialize_element(&self.trips_by_id)?;
        seq.serialize_element(&self.routes_by_id)?;

        seq.end()
    }
}

impl <'r> GTFSData<'r> {
    pub fn new() -> GTFSData<'r> {
        GTFSData {
            stop_times_arena: Arena::new(),
            stop_departures: RefCell::new(HashMap::new()),
            transfers: HashMap::new(),
            stops_by_id: HashMap::new(),
            trips_by_id: HashMap::new(),
            routes_by_id: HashMap::new(),
        }
    }

    pub fn get_route_for_trip(&self, trip_id: &TripId) -> Option<&Route> {
        self.trips_by_id.get(trip_id).and_then(|trip| self.routes_by_id.get(&trip.route_id))
    }

    pub fn borrow_stop_departures(&self) -> Result<Ref<HashMap<StopId, Vec<&'r[StopTime]>>>, BorrowError> {
      self.stop_departures.try_borrow()
    }

    pub fn load_stops_by_id(&mut self, source: &GTFSSource) -> Result<(), Box<dyn Error>> {
        let mut rdr = source.open_csv("stops.txt")?;
        for result in rdr.deserialize() {
            let stop: Stop = result?;
            self.stops_by_id.insert(stop.stop_id.clone(), stop);
        }
        Ok(())
    }

    pub fn load_trips_by_id(&mut self, source: &GTFSSource) -> Result<(), Box<dyn Error>> {
        let mut rdr = source.open_csv("trips.txt")?;
        for result in rdr.deserialize() {
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

    pub fn departure_lookup(&'r self, period: Period, source: &GTFSSource,) -> Result<(), Box<dyn Error>> {
        // let stop_times_arena = Arena::new();
        let sunday_services = source.get_sunday_services()?;
        eprintln!("{} services", sunday_services.len());
        let available_trips = source.get_trips(None, sunday_services, None)?;
        let available_trips: HashMap<TripId, Trip> = available_trips.into_iter().map(|trip| (trip.trip_id, trip)).collect();

        let mut rdr = source.open_csv("stop_times.txt")?;
        let mut iter = SuperIter {
            records: rdr.deserialize().peekable(),
            trip_id: None,
        };
        let mut stop_departures = self.stop_departures.try_borrow_mut()?;
        let mut count = 0;
        while let Some(result) = iter.next() {
            let (trip_id, stops) = result?;
            if available_trips.contains_key(&trip_id) {
                let stops = stops.skip_while(|result| result.iter().any(|stop| !period.contains(stop.departure_time)));
                let stops: &'r[StopTime] = self.stop_times_arena.alloc_extend(stops.flatten());
                if stops.len() > 0 {
                    count += 1;
                }
                for start_index in 0..stops.len() {
                    let departures_from_stop = stop_departures.entry(stops[start_index].stop_id).or_default();
                    departures_from_stop.push(&stops[start_index..]);
                }
            }
        }
        eprintln!("{} trips", count);
        eprintln!("{} departures allocated, leaving from {} stops", self.stop_times_arena.len(), stop_departures.len());

        Ok(())
    }

    pub fn stops_by_parent_id(&self, parent: &StopId) -> Vec<StopId> {
        let mut stops = vec![];
        for stop in self.stops_by_id.values() {
            if Some(*parent) == stop.parent_station {
                stops.push(stop.stop_id);
            }
        }
        stops
    }
}
