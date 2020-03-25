use std::collections::{BinaryHeap, HashMap, BTreeSet, HashSet};
use crate::gtfstime::{Time, Period};
use crate::gtfs::*;
use crate::gtfs::db::GTFSData;
// use typed_arena::Arena;
use std::cmp::Ordering;


pub struct JourneyGraphPlotter<'r: 's, 's> {
  // root_node: &'node Node,
  period: Period, // Search of journeys is within this period
  queue: BinaryHeap<QueueItem<'r>>,
  enqueued_trips: HashSet<TripId>,
  stops: HashMap<StopId, BTreeSet<Time>>, 
  // nodes: Arena<Node>,
  trips_from_stops: std::cell::Ref<'s, HashMap<StopId, Vec<&'r[StopTime]>>>,
  // transfers: &'r HashMap<StopId, Vec<Transfer>>,
  data: &'r GTFSData<'r>,
}

impl <'r: 's, 's> JourneyGraphPlotter<'r, 's> {
  pub fn new(period: Period, data: &'r GTFSData<'r>) -> Result<JourneyGraphPlotter<'r, 's>, std::cell::BorrowError> {
    Ok(JourneyGraphPlotter {
      period: period,
      queue: std::collections::BinaryHeap::new(),
      enqueued_trips: HashSet::new(),
      stops: HashMap::new(),
      trips_from_stops: data.borrow_stop_departures()?,
      // transfers: &data.transfers,
      data: data,
    })
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
      QueueItemVariant::Connection => None,
      QueueItemVariant::StopOnTrip{route} => {
        Some(&route.route_short_name)
      },
    }
  }

  pub fn get_route_type(&self) -> Option<RouteType> {
    match self.variant {
      QueueItemVariant::Connection => None,
      QueueItemVariant::StopOnTrip{route} => {
        Some(route.route_type)
      },
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
  StopOnTrip { route: &'r Route },
  Connection,
}

impl<'r, 's> Iterator for JourneyGraphPlotter<'r, 's> {
  type Item = (QueueItem<'r>, bool);

  fn next(&mut self) -> Option<Self::Item> {
    if let Some(item) = self.queue.pop() {
      if self.period.contains(item.arrival_time) {
        let fastest = self.process_queue_item(&item);
        return Some((item, fastest))
      }
    }
    None
  }
}

impl <'node, 'r, 's> JourneyGraphPlotter<'r, 's> {
  pub fn add_origin(&mut self, fake_stop: &'r Stop, origin: &'r Stop) {
    self.queue.push(QueueItem {
      departure_time: self.period.start(),
      arrival_time: self.period.start(),
      from_stop: &fake_stop,
      to_stop: origin,
      variant: QueueItemVariant::Connection,
    });
  }

  fn process_queue_item(&mut self, item: &QueueItem) -> bool {
    // // create a new node for this item
    // let node = Node {
    //   arrival_time: item.arrival_time,
    // };
    // get existing nodes for this stop
    let nodes = self.stops.entry(item.to_stop.stop_id).or_default();
    let new_earliest_arrival = nodes.is_empty() || Some(&item.arrival_time) < nodes.iter().next();
    // add node in ordered by arrival_time
    nodes.insert(item.arrival_time);
    if new_earliest_arrival { // if this changes the earliest arrival time for this stop, we possibly have new connections / trips
      match item.variant {
        QueueItemVariant::StopOnTrip { route: _route } => {
          let mut to_add = vec![];
          for transfer in self.transfers_from(item.to_stop.stop_id) {
            to_add.push(QueueItem {
              from_stop: self.data.get_stop(&transfer.from_stop_id).unwrap(),
              to_stop: self.data.get_stop(&transfer.to_stop_id).unwrap(),
              departure_time: item.arrival_time,
              arrival_time: item.arrival_time + transfer.min_transfer_time.unwrap_or_default(),
              variant: QueueItemVariant::Connection,
            });
          }
          self.queue.extend(to_add);
        },
        QueueItemVariant::Connection => {
          let mut to_add = vec![];
          for stops in self.trips_from(item.to_stop.stop_id, self.period.with_start(item.arrival_time)) {
            let trip_id = stops[0].trip_id;
            let route = self.data.get_route_for_trip(&trip_id).expect(&format!("to have found a route for trip {}", trip_id));
            if !self.enqueued_trips.contains(&trip_id) { // make sure we only add each trip once
              for window in stops.windows(2) {
                if let [from_stop, to_stop] = window {
                  to_add.push(QueueItem {
                    from_stop: self.data.get_stop(&from_stop.stop_id).unwrap(),
                    to_stop: self.data.get_stop(&to_stop.stop_id).unwrap(),
                    departure_time: from_stop.arrival_time,
                    arrival_time: to_stop.arrival_time,
                    variant: QueueItemVariant::StopOnTrip{ route },
                  });
                } else {
                  panic!("Bad window");
                }
              }
            }
          }
          self.queue.extend(to_add);
        },
      }
      true
    } else {
      false
    }
  }

  /// finds all trips leaving a stop within a time period, includes the stop time for that stop and all following stops
  fn trips_from(&self, stop: StopId, period: Period) -> impl Iterator<Item = &&'r[StopTime]> {
    let departures = self.trips_from_stops.get(&stop).map(|vec| vec.iter()).unwrap_or([].iter());
    departures.filter(move |stop_time: &&&[StopTime]| period.contains(stop_time[0].departure_time))
  }

  /// finds all connections from a stop
  fn transfers_from(&self, stop: StopId) -> impl Iterator<Item = &Transfer> {
    self.data.get_transfers(&stop).map(|vec| vec.iter()).unwrap_or([].iter())
  }
}
