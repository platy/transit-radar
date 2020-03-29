use std::collections::{BinaryHeap, HashMap, BTreeSet, HashSet};
use crate::gtfstime::{Time, Period};
use crate::gtfs::*;
use crate::gtfs::db::GTFSData;
// use typed_arena::Arena;
use std::cmp::Ordering;
use std::ops::Range;


pub struct JourneyGraphPlotter<'r: 's, 's> {
  period: Period, // Search of journeys is within this period
  queue: BinaryHeap<QueueItem<'r>>,
  /// items which were skipped earlier as it didn't seem they would be part of any minimum span but now are, these have already been processed and are iterated before any more processing from the queue takes place
  catch_up: BinaryHeap<QueueItem<'r>>, 
  enqueued_trips: HashSet<TripId>,
  /// trips which so far have only gotten us late to stops, but they may end up leading to useful stops - will need to clean this up when the last stop in a trip is reached as it will probably grow badly
  slow_trips: HashMap<TripId, Vec<QueueItem<'r>>>,
  stops: HashMap<StopId, BTreeSet<Time>>, 
  trips_from_stops: &'s HashMap<StopId, Vec<Range<usize>>>,
  data: &'r GTFSData,
  route_types: HashSet<RouteType>,
}

impl <'r: 's, 's> JourneyGraphPlotter<'r, 's> {
  pub fn new(period: Period, data: &'r GTFSData) -> JourneyGraphPlotter<'r, 's> {
    JourneyGraphPlotter {
      period: period,
      queue: BinaryHeap::new(),
      catch_up: BinaryHeap::new(),
      enqueued_trips: HashSet::new(),
      slow_trips: HashMap::new(),
      stops: HashMap::new(),
      trips_from_stops: data.borrow_stop_departures(),
      // transfers: &data.transfers,
      data: data,
      route_types: HashSet::new(),
    }
  }
}

// #[derive(Debug, Eq, PartialEq, Ord, PartialOrd)]
// struct Node {
//   arrival_time: Time,
// }

#[derive(Debug)]
pub struct QueueItem<'r> {
  pub departure_time: Time,
  pub arrival_time: Time,
  pub from_stop: &'r Stop,
  pub to_stop: &'r Stop,
  pub variant: QueueItemVariant<'r>,
}

impl<'r> QueueItem<'r> {
  pub fn get_route_name(&self) -> Option<&'r str> {
    match self.variant {
      QueueItemVariant::OriginStation => None,
      QueueItemVariant::Transfer => None,
      QueueItemVariant::Connection{trip_id: _, route} => {
        Some(&route.route_short_name)
      },
      QueueItemVariant::StopOnTrip{trip_id: _, route} => {
        Some(&route.route_short_name)
      },
    }
  }

  pub fn get_route_type(&self) -> Option<RouteType> {
    match self.variant {
      QueueItemVariant::OriginStation => None,
      QueueItemVariant::Transfer => None,
      QueueItemVariant::Connection{trip_id: _, route: _} => None,
      QueueItemVariant::StopOnTrip{trip_id: _, route} => Some(route.route_type),
    }
  }
}

/// The ordering on the queue items puts those with the earliest arrival times as the greatest,
/// so that they will be highest priority in the BinaryHeap, then all the other fields need to be
/// taken into account for a full ordering
impl <'node, 'r> Ord for QueueItem<'r> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.arrival_time.cmp(&other.arrival_time).reverse().then_with(||
          self.from_stop.stop_id.cmp(&other.from_stop.stop_id).then(
            self.to_stop.stop_id.cmp(&other.to_stop.stop_id).then(
              self.variant.cmp(&other.variant)))
        )
    }
}

impl <'node, 'r> PartialOrd for QueueItem<'r> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl <'node, 'r> PartialEq for QueueItem<'r> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl <'node, 'r> Eq for QueueItem<'r> {}

#[derive(Debug, Ord, PartialOrd, PartialEq, Eq)]
pub enum QueueItemVariant<'r> {
  StopOnTrip { 
    trip_id: TripId, 
    route: &'r Route 
  },
  Connection { 
    trip_id: TripId, 
    route: &'r Route 
  },
  Transfer,
  OriginStation,
}

impl<'r, 's> Iterator for JourneyGraphPlotter<'r, 's> {
  type Item = QueueItem<'r>;

  fn next(&mut self) -> Option<Self::Item> {
    if let Some(item) = self.catch_up.pop() {
      return Some(item)
    }
    while let Some(item) = self.queue.pop() {
      if !self.period.contains(item.arrival_time) {
        return None // we ran out of the time period
      } else {
        if let Some(item) = self.process_queue_item(item) { // when a queue item is processed and emitted, it could have added items to catch up which should go before it
          return Some(item) // we found something that's worth drawing
        }
      }
    }
    None // we exhausted the queues
  }
}

impl <'node, 'r, 's> JourneyGraphPlotter<'r, 's> {
  pub fn add_origin_station(&mut self, origin: &'r Stop) {
    self.queue.push(QueueItem {
      departure_time: self.period.start(),
      arrival_time: self.period.start(),
      from_stop: self.data.fake_stop(),
      to_stop: origin,
      variant: QueueItemVariant::OriginStation,
    });
  }

  pub fn add_route_types(&mut self, route_types: impl IntoIterator<Item = RouteType>) {
    self.route_types.extend(route_types);
  }

  fn process_queue_item(&mut self, item: QueueItem<'r>) -> Option<QueueItem<'r>> {
    // get existing nodes for this stop
    let nodes = self.stops.entry(item.to_stop.stop_id).or_default();
    let new_earliest_arrival = nodes.is_empty() || Some(&item.arrival_time) < nodes.iter().next();
    // add node in ordered by arrival_time
    nodes.insert(item.arrival_time);
    if new_earliest_arrival { // if this changes the earliest arrival time for this stop, we possibly have new connections / trips
      match item.variant {
        QueueItemVariant::StopOnTrip { trip_id, route: _route } => {
          let mut to_add = vec![];
          for transfer in self.transfers_from(item.to_stop.stop_id) {
            to_add.push(QueueItem {
              from_stop: self.data.get_stop(&transfer.from_stop_id).unwrap(),
              to_stop: self.data.get_stop(&transfer.to_stop_id).unwrap(),
              departure_time: item.arrival_time,
              arrival_time: item.arrival_time + transfer.min_transfer_time.unwrap_or_default(),
              variant: QueueItemVariant::Transfer,
            });
          }
          self.queue.extend(to_add);
          // if this now made some slow stops on the trip relevant, they should be emitted as well
          let slow_trips = self.slow_trips.remove(&trip_id);
          if let Some(slow_trips) = slow_trips {
            self.catch_up.extend(slow_trips);
            self.catch_up.push(item);
            Some(self.catch_up.pop().expect("item from catch up queue"))
          } else {
            Some(item)
          }
        },
        QueueItemVariant::Connection { trip_id: _, route: _ } => {
          panic!("Unexpected");
        },
        QueueItemVariant::Transfer => {
          let mut to_add = vec![];
          for stops_range in self.trips_from(item.to_stop.stop_id, self.period.with_start(item.arrival_time)) {
            let stops = self.data.stops(stops_range.clone());
            let trip_id = stops[0].trip_id;
            // check that route type is allowed
            if self.data.get_route_for_trip(&trip_id).iter().any(|route| self.route_types.contains(&route.route_type)) {
              // make sure we only add each trip once
              if !self.enqueued_trips.contains(&trip_id) { 
                let route = self.data.get_route_for_trip(&trip_id).expect(&format!("to have found a route for trip {}", trip_id));
                // enqueue connection (transfer + wait)
                to_add.push(QueueItem{
                  from_stop: item.from_stop,
                  to_stop: item.to_stop,
                  departure_time: item.departure_time,
                  arrival_time: stops[0].departure_time,
                  variant: QueueItemVariant::Connection{ trip_id, route },
                });
                let mut is_first = true;
                for window in stops.windows(2) {
                  if let [from_stop, to_stop] = window {
                    to_add.push(QueueItem {
                      from_stop: self.data.get_stop(&from_stop.stop_id).unwrap(),
                      to_stop: self.data.get_stop(&to_stop.stop_id).unwrap(),
                      departure_time: if is_first { from_stop.departure_time } else { from_stop.arrival_time },
                      arrival_time: to_stop.arrival_time,
                      variant: QueueItemVariant::StopOnTrip{ trip_id, route },
                    });
                  } else {
                    panic!("Bad window");
                  }
                  is_first = false;
                }
              }
            }
          }
          self.queue.extend(to_add);
          // we don't emit transfers unless they are to a new station
          if item.from_stop.parent_station == item.to_stop.parent_station {
            None
          } else {
            Some(item)
          }
        },
        QueueItemVariant::OriginStation => {
          let origin_stops = self.data.stops_by_parent_id(&item.to_stop.stop_id);
          let to_add: Vec<QueueItem> = origin_stops.into_iter().map(|stop_id| {
            let stop = self.data.get_stop(&stop_id).unwrap();
            // immediately transfer to all the stops of this origin station
            QueueItem {
              from_stop: item.from_stop,
              to_stop: stop,
              departure_time: item.departure_time,
              arrival_time: item.arrival_time,
              variant: QueueItemVariant::Transfer,
            }
          }).collect();
          self.queue.extend(to_add);
          Some(item)
        }
      }
    } else {
      match item.variant {
        // late arrival by trip, we want it if this trip will take us somewhere new eventually, so save it for later
        QueueItemVariant::StopOnTrip { trip_id, route: _ } => {
          let slow_trip = self.slow_trips.entry(trip_id).or_default();
          slow_trip.push(item);
        },
        QueueItemVariant::Connection { trip_id, route: _ } => {
          let slow_trip = self.slow_trips.entry(trip_id).or_default();
          slow_trip.push(item);
        },
        _ => () // late arrival by transfer - drop it
      }
      None // the item will not be emitted
    }
  }

  /// finds all trips leaving a stop within a time period, includes the stop time for that stop and all following stops
  fn trips_from(&self, stop: StopId, period: Period) -> impl Iterator<Item = &Range<usize>> {
    let departures: Option<&Vec<Range<usize>>> = self.trips_from_stops.get(&stop);
    departures.map(|vec| vec.iter()).unwrap_or([].iter()).filter(move |stop_range: &&Range<usize>| period.contains(self.data.stop(stop_range.start).departure_time))
  }

  /// finds all connections from a stop
  fn transfers_from(&self, stop: StopId) -> impl Iterator<Item = &Transfer> {
    self.data.get_transfers(&stop).map(|vec| vec.iter()).unwrap_or([].iter())
  }
}
