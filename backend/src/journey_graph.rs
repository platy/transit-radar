use std::collections::{BinaryHeap, HashMap, BTreeSet, HashSet};
use crate::gtfstime::{Time, Period};
use crate::gtfs::{StopId, TripId, StopTime, Transfer};
// use typed_arena::Arena;
use std::cmp::Ordering;


pub struct JourneyGraphPlotter<'r> {
  // root_node: &'node Node,
  period: Period, // Search of journeys is within this period
  queue: BinaryHeap<QueueItem>,
  enqueued_trips: HashSet<TripId>,
  stops: HashMap<StopId, BTreeSet<Time>>, 
  // nodes: Arena<Node>,
  trips_from_stops: HashMap<StopId, Vec<&'r[StopTime]>>,
  transfers: HashMap<StopId, Vec<Transfer>>,
}

impl <'r> JourneyGraphPlotter<'r> {
  pub fn new(period: Period, trips_from_stops: HashMap<StopId, Vec<&'r[StopTime]>>, transfers: HashMap<StopId, Vec<Transfer>>) -> JourneyGraphPlotter {
    JourneyGraphPlotter {
      period: period,
      queue: std::collections::BinaryHeap::new(),
      enqueued_trips: HashSet::new(),
      stops: HashMap::new(),
      // nodes: Arena<Node>,
      trips_from_stops: trips_from_stops,
      transfers: transfers,
    }
  }
}

// #[derive(Debug, Eq, PartialEq, Ord, PartialOrd)]
// struct Node {
//   arrival_time: Time,
// }

#[derive(Debug)]
struct QueueItem {
  arrival_time: Time,
  // from_node: &'node Node,
  to_stop_id: StopId, // ?? not sure
  variant: QueueItemVariant,
}

/// The ordering on the queue items puts those with the earliest arrival times as the greatest,
/// so that they will be highest priority in the BinaryHeap, then all the other fields need to be
/// taken into account for a full ordering
impl <'node, 'r> Ord for QueueItem {
    fn cmp(&self, other: &Self) -> Ordering {
        self.arrival_time.cmp(&other.arrival_time).reverse().then_with(||
          self.to_stop_id.cmp(&other.to_stop_id).then(self.variant.cmp(&other.variant))
        )
    }
}

impl <'node, 'r> PartialOrd for QueueItem {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl <'node, 'r> PartialEq for QueueItem {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl <'node, 'r> Eq for QueueItem {}

#[derive(Debug, Ord, PartialOrd, PartialEq, Eq)]
enum QueueItemVariant {
  StopOnTrip { trip: TripId },
  Connection,
}

impl <'node, 'r> JourneyGraphPlotter<'r> {
  pub fn run(&mut self, origin: StopId) {
    self.queue.push(QueueItem {
      arrival_time: self.period.start(),
      to_stop_id: origin,
      variant: QueueItemVariant::Connection,
    });
    while let Some(item) = self.queue.pop() {
      self.process_queue_item(item);
    }
  }

  fn process_queue_item(&mut self, item: QueueItem) {
    if !self.period.contains(item.arrival_time) {
      // end search
      return
    }
    // // create a new node for this item
    // let node = Node {
    //   arrival_time: item.arrival_time,
    // };
    // get existing nodes for this stop
    let nodes = self.stops.entry(item.to_stop_id).or_default();
    let new_earliest_arrival = nodes.is_empty() || Some(&item.arrival_time) < nodes.iter().next();
    // add node in ordered by arrival_time
    nodes.insert(item.arrival_time);
    if new_earliest_arrival { // if this changes the earliest arrival time for this stop, we possibly have new connections / trips
      match item.variant {
        QueueItemVariant::StopOnTrip { trip } => {
          println!("{} Arrived at {} with {:?}", item.arrival_time, &item.to_stop_id, trip);
          let mut to_add = vec![];
          for transfer in self.transfers_from(item.to_stop_id) {
            println!("Using connection {:?}", transfer);
            to_add.push(QueueItem {
              to_stop_id: transfer.to_stop_id,
              arrival_time: item.arrival_time + transfer.min_transfer_time.unwrap_or_default(),
              variant: QueueItemVariant::Connection,
            });
          }
          self.queue.extend(to_add);
        },
        QueueItemVariant::Connection => {
          println!("{} Connected to {}", item.arrival_time, &item.to_stop_id);
          let mut to_add = vec![];
          for stops in self.trips_from(item.to_stop_id, self.period.with_start(item.arrival_time)) {
            let trip_id = stops[0].trip_id;
            if !self.enqueued_trips.contains(&trip_id) { // make sure we only add each trip once
              println!("Taking trip {:?}", trip_id);
              for window in stops.windows(2) {
                if let [_from_stop, to_stop] = window {
                  to_add.push(QueueItem {
                    to_stop_id: to_stop.stop_id,
                    arrival_time: to_stop.arrival_time,
                    variant: QueueItemVariant::StopOnTrip{ trip: trip_id },
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
    } else {
      println!("Arrived at {} again", &item.to_stop_id);
    }
  }

  /// finds all trips leaving a stop within a time period, includes the stop time for that stop and all following stops
  fn trips_from(&self, stop: StopId, period: Period) -> impl Iterator<Item = &&'r[StopTime]> {
    let departures = self.trips_from_stops.get(&stop).map(|vec| vec.iter()).unwrap_or([].iter());
    departures.filter(move |stop_time: &&&[StopTime]| period.contains(stop_time[0].departure_time))
  }

  /// finds all connections from a stop
  fn transfers_from(&self, stop: StopId) -> impl Iterator<Item = &Transfer> {
    self.transfers.get(&stop).expect("should have transfers for stop").iter()
  }
}
