use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};
use std::cmp::Ordering;
use std::fmt;
use crate::gtfs::*;
use crate::gtfs::db::{GTFSData, TripStopRef};


pub struct JourneyGraphPlotter<'r> {
  period: Period, // Search of journeys is within this period
  services: HashSet<ServiceId>, // these services are searched
  queue: BinaryHeap<QueueItem<'r>>,
  /// items which were skipped earlier as it didn't seem they would be part of any minimum span but now are, these have already been processed and ordered and are iterated before any more processing from the queue takes place
  catch_up: VecDeque<Item<'r>>, 
  enqueued_trips: HashSet<TripId>,
  /// trips which so far have only gotten us late to stops, but they may end up leading to useful stops - will need to clean this up when the last stop in a trip is reached as it will probably grow badly
  slow_trips: HashMap<TripId, Vec<QueueItem<'r>>>,
  // stops that have been arrived at and the earliest time they are arrived at
  stops: HashMap<StopId, Time>, 
  emitted_stations: HashSet<StopId>,
  data: &'r GTFSData,
  route_types: HashSet<RouteType>,
}

impl <'r> JourneyGraphPlotter<'r> {
  pub fn new(day: Day, period: Period, data: &'r GTFSData) -> JourneyGraphPlotter<'r> {
    JourneyGraphPlotter {
      period: period,
      services: data.services_of_day(day),
      queue: BinaryHeap::new(),
      catch_up: VecDeque::new(),
      enqueued_trips: HashSet::new(),
      slow_trips: HashMap::new(),
      stops: HashMap::new(),
      emitted_stations: HashSet::new(),
      data: data,
      route_types: HashSet::new(),
    }
  }
}

struct QueueItem<'r> {
  arrival_time: Time,
  to_stop: &'r Stop,
  variant: QueueItemVariant<'r>,
}

impl<'r> fmt::Debug for QueueItem<'r> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("QueueItem")
     .field("to_stop", &format!("{} ({:?}{})", self.to_stop.stop_name, self.to_stop.stop_id, if self.to_stop.parent_station.is_none() { "*" } else { "" }))
     .field("arrival_time", &self.arrival_time)
     .field("variant", &self.variant)
     .finish()
  }
}

/// The ordering on the queue items puts those with the earliest arrival times as the greatest,
/// so that they will be highest priority in the BinaryHeap, then all the other fields need to be
/// taken into account for a full ordering
impl <'node, 'r> Ord for QueueItem<'r> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.arrival_time.cmp(&other.arrival_time).reverse().then_with(||
          self.to_stop.stop_id.cmp(&other.to_stop.stop_id).then(
            self.variant.cmp(&other.variant)))
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

#[derive(Debug)]
pub enum Item<'r> {
  JourneySegment {
    departure_time: Time,
    arrival_time: Time,
    from_stop: &'r Stop,
    to_stop: &'r Stop,
  },
  ConnectionToTrip {
    departure_time: Time,
    arrival_time: Time,
    from_stop: &'r Stop,
    to_stop: &'r Stop,
    route_name: &'r str,
    route_type: RouteType,
  },
  SegmentOfTrip {
    departure_time: Time,
    arrival_time: Time,
    from_stop: &'r Stop,
    to_stop: &'r Stop,
    trip_id: TripId,
    route_name: &'r str,
    route_type: RouteType,
  },
  Station {
    stop: &'r Stop,
    earliest_arrival: Time,
  }
}

#[derive(Debug, Ord, PartialOrd, PartialEq, Eq)]
enum QueueItemVariant<'r> {
  StopOnTrip { 
    departure_time: Time,
    from_stop: &'r Stop,
    trip_id: TripId, 
    route: &'r Route,
    previous_arrival_time: Time, // arrival at the from stop
    next_departure_time: Time, // departure from the to stop
  },
  Connection { 
    departure_time: Time,
    from_stop: &'r Stop,
    trip_id: TripId, 
    route: &'r Route 
  },
  Transfer {
    departure_time: Time,
    from_stop: &'r Stop,
  },
  OriginStation,
}

impl<'r> QueueItemVariant<'r> {
  fn get_from_stop(&self) -> Option<&'r Stop> {
    match self {
      QueueItemVariant::Connection { 
        departure_time: _,
        from_stop,
        trip_id: _, 
        route: _ 
      } => Some(from_stop),
      QueueItemVariant::StopOnTrip { 
        departure_time: _,
        from_stop,
        trip_id: _, 
        route: _,
        previous_arrival_time: _,
        next_departure_time: _,
      } => Some(from_stop),
      QueueItemVariant::Transfer {
        departure_time: _,
        from_stop,
      } => Some(from_stop),
      QueueItemVariant::OriginStation => None,
    }
  }
}

impl<'r> Iterator for JourneyGraphPlotter<'r> {
  type Item = Item<'r>;

  fn next(&mut self) -> Option<Self::Item> {
    if let Some(item) = self.catch_up.pop_front() {
      return Some(item);
    }
    let mut next_block = self.next_block().into_iter();
    if let Some(first) = next_block.next() {
      self.catch_up.extend(next_block);
      Some(first)
    } else {
      None // we exhausted the queues
    }
  }
}

impl <'node, 'r> JourneyGraphPlotter<'r> {
  // returns the next items to be emitted in order, or empty if there are no more
  fn next_block(&mut self) -> Vec<Item<'r>> {
    while let Some(item) = self.queue.pop() {
      if !self.period.contains(item.arrival_time) {
        return vec![]; // we ran out of the time period
      } else {
        let processed: Vec<Item<'r>> = self.process_queue_item(item).into_iter().flat_map(|item| {
          let mut to_emit = vec![];
          // if this arrives at a new station, emit that first
          if self.emitted_stations.insert(item.to_stop.station_id()) {
            to_emit.push(Item::Station {
              stop: item.to_stop,
              earliest_arrival: item.arrival_time,
            });
          }
          if let Some(item) = self.convert_item(item) {
            to_emit.push(item);
          }
          to_emit // we found something that's worth drawing
        }).collect();
        if !processed.is_empty() {
          return processed
        }
      }
    }
    vec![]
  }

  fn convert_item(&self, QueueItem {
      to_stop,
      mut arrival_time,
      variant,
    }: QueueItem<'r>) -> Option<Item<'r>> {
      match variant {
        QueueItemVariant::OriginStation => None,
        QueueItemVariant::Transfer {
          from_stop,
          departure_time,
        } => {
          Some(Item::JourneySegment {
            from_stop,
            to_stop,
            departure_time,
            arrival_time,
          })
        },
        QueueItemVariant::Connection{
          trip_id: _, 
          route,
          from_stop,
          departure_time,
        } => Some(Item::ConnectionToTrip {
          from_stop,
          to_stop,
          departure_time,
          arrival_time,
          route_name: &route.route_short_name,
          route_type: route.route_type,
        }),
        QueueItemVariant::StopOnTrip {
          trip_id, 
          route, 
          previous_arrival_time, 
          next_departure_time,
          from_stop,
          mut departure_time,
        } => {
          // we don't show the stop time at each station along the trip, so we use one time
          // at each stop. If the stop is the earliest arrival at the station, we use the
          // arrival time, if not we use the departure time
          if Some(previous_arrival_time) == self.earliest_arrival_at(from_stop.stop_id) {
            departure_time = previous_arrival_time;
          }
          if Some(arrival_time) > self.earliest_arrival_at(to_stop.stop_id) {
            arrival_time = next_departure_time;
          }
          Some(Item::SegmentOfTrip {
            from_stop,
            to_stop,
            departure_time,
            arrival_time,
            trip_id,
            route_name: &route.route_short_name,
            route_type: route.route_type,
          })
        },
      }
  }

  pub fn add_origin_station(&mut self, origin: &'r Stop) {
    self.queue.push(QueueItem {
      arrival_time: self.period.start(),
      to_stop: origin,
      variant: QueueItemVariant::OriginStation,
    });
  }

  pub fn add_route_type(&mut self, route_type: RouteType) {
    self.route_types.insert(route_type);
  }

  fn enqueue_transfers_from_stop(&mut self, stop: &'r Stop, departure_time: Time) {
    let mut to_add = vec![];
    for transfer in self.transfers_from(stop.stop_id) {
      if !self.stops.contains_key(&transfer.to_stop_id) {
        to_add.push(QueueItem {
          to_stop: self.data.get_stop(&transfer.to_stop_id).unwrap(),
          arrival_time: departure_time + transfer.min_transfer_time.unwrap_or_default(),
          variant: QueueItemVariant::Transfer {
            from_stop: stop,
            departure_time: departure_time,
          },
        });
      }
    }
    self.queue.extend(to_add);
  }

  fn enqueue_transfers_from_station(&mut self, station: &'r Stop, departure_time: Time) {
    let mut to_add = vec![];
    for transfer in self.transfers_from(station.station_id()) {
      // parent stations transfer to parents, so transfer to the children instead
      for to_stop_id in self.data.stops_by_parent_id(&transfer.to_stop_id) {
        if !self.stops.contains_key(&transfer.to_stop_id) {
          to_add.push(QueueItem {
            to_stop: self.data.get_stop(&to_stop_id).unwrap(),
            arrival_time: departure_time + transfer.min_transfer_time.unwrap_or_default(),
            variant: QueueItemVariant::Transfer {
              from_stop: station,
              departure_time: departure_time,
            },
          });
        }
      }
    }
    self.queue.extend(to_add);
  }

  fn enqueue_immediate_transfers_to_children_of(&mut self, stop: &'r Stop, arrival_time: Time) {
    let origin_stops = self.data.stops_by_parent_id(&stop.stop_id);
    let to_add: Vec<QueueItem> = origin_stops.into_iter().map(|stop_id| {
      let child_stop = self.data.get_stop(&stop_id).unwrap();
      // immediately transfer to all the stops of this origin station
      QueueItem {
        to_stop: child_stop,
        arrival_time: arrival_time,
        variant: QueueItemVariant::Transfer {
          from_stop: stop,
          departure_time: arrival_time,
        },
      }
    }).collect();
    self.queue.extend(to_add);
  }

  fn enqueue_connections_and_trips(&mut self, item: &QueueItem<'r>, from_stop: &'r Stop, departure_time: Time) -> bool {
    let mut to_add = vec![];
    for stop_ref in self.trips_from(item.to_stop.stop_id, self.period.with_start(item.arrival_time)) {
      let stops = self.data.stop_times(&stop_ref);
      let trip_id = stops[0].trip_id;
      let mut trip_to_add = vec![];
      // check that route type is allowed
      if self.route_types.contains(&self.data.get_route_for_trip(&trip_id).route_type) {
        let route = self.data.get_route_for_trip(&trip_id);
        // enqueue connection (transfer + wait)
        trip_to_add.push(QueueItem{
          to_stop: item.to_stop,
          arrival_time: stops[0].departure_time,
          variant: QueueItemVariant::Connection{ 
            trip_id, 
            route,
            from_stop: from_stop,
            departure_time: departure_time,
           },
        });
        for window in stops.windows(2) {
          if let [from_stop, to_stop] = window {
            trip_to_add.push(QueueItem {
              to_stop: self.data.get_stop(&to_stop.stop_id).unwrap(),
              arrival_time: to_stop.arrival_time,
              variant: QueueItemVariant::StopOnTrip{ 
                trip_id, 
                route, 
                previous_arrival_time: from_stop.arrival_time, 
                next_departure_time: to_stop.departure_time,
                from_stop: self.data.get_stop(&from_stop.stop_id).unwrap(),
                departure_time: from_stop.departure_time,
              },
            });
          } else {
            panic!("Bad window");
          }
        }
      }
      to_add.push((trip_id, trip_to_add));
    }
    let mut extended = false;
    for (trip_id, to_add) in to_add {
      // make sure we only add each trip once
      if self.enqueued_trips.insert(trip_id) {
        extended = true;
        self.queue.extend(to_add);
      }
    }
    extended
  }

  fn earliest_arrival_at(&self, stop_id: StopId) -> Option<Time> {
    self.stops.get(&stop_id)
        .cloned()
  }

  fn filter_slow_trip(&mut self, slow_trip: Vec<QueueItem<'r>>) -> Vec<QueueItem<'r>> {
    // this trip became useful but it might be that we don't board at the first stop where we encountered it, we should board at the stop we can get to the earliest, not the earliest we can board this trip
    let boarding_idx = slow_trip.iter()
      .enumerate()
      .filter_map(|(i, item)| 
        // Each item must only be a StopOnTrip or a Connection
        self.earliest_arrival_at(item.variant.get_from_stop().expect("A slow trip must only contain connections and stops, no transfers or origins").stop_id)
        .map(|time| (i, time))
      ).min_by_key(|(_i, first_arrival)| *first_arrival).map(|(i, _t)| i).unwrap_or(0);
    slow_trip.into_iter().skip(boarding_idx).collect()
  }

  fn set_arrival_time(&mut self, stop_id: StopId, new_arrival_time: Time) -> bool {
    if self.stops.get(&stop_id).iter().all(|&&previous_earliest_arrival| new_arrival_time < previous_earliest_arrival) {
      self.stops.insert(stop_id, new_arrival_time);
      true
    } else {
      false
    }
  }

  fn process_queue_item(&mut self, item: QueueItem<'r>) -> Vec<QueueItem<'r>> {
    if self.set_arrival_time(item.to_stop.stop_id, item.arrival_time) { // if this changes the earliest arrival time for this stop, we possibly have new connections / trips
      match item.variant {
        QueueItemVariant::StopOnTrip { 
          trip_id, 
          route: _route, 
          previous_arrival_time: _, 
          next_departure_time: _,
          from_stop: _,
          departure_time: _,
        } => {
          self.enqueue_transfers_from_stop(item.to_stop, item.arrival_time);
          self.enqueue_transfers_from_station(item.to_stop, item.arrival_time);
          // only emit if we got to a new station
          if !self.emitted_stations.contains(&item.to_stop.station_id()) {
            // if this now made some slow stops on the trip relevant, they should be emitted as well
            let slow_trip = self.slow_trips.remove(&trip_id);
            if let Some(slow_trip) = slow_trip {
              let mut to_emit = self.filter_slow_trip(slow_trip);
              to_emit.push(item);
              to_emit
            } else {
              vec![item]
            }
          } else {
            vec![]
          }
        },
        QueueItemVariant::Connection { 
          trip_id: _, 
          route: _ ,
          from_stop: _,
          departure_time: _,
        } => {
          panic!("Unexpected");
        },
        QueueItemVariant::Transfer {
          from_stop,
          departure_time,
        } => {
          let extended = self.enqueue_connections_and_trips(&item, &from_stop, departure_time);
          // we don't emit transfers unless they are to a new station which accesses other trips
          if !extended || from_stop.parent_station == item.to_stop.parent_station {
            vec![]
          } else {
            vec![item]
          }
        },
        QueueItemVariant::OriginStation => {
          self.enqueue_immediate_transfers_to_children_of(item.to_stop, item.arrival_time);
          self.enqueue_transfers_from_station(item.to_stop, item.arrival_time);
          vec![item]
        }
      }
    } else {
      match item.variant {
        // late arrival by trip, we want it if this trip will take us somewhere new eventually, so save it for later
        QueueItemVariant::StopOnTrip { 
          trip_id, 
          route: _, 
          previous_arrival_time: _, 
          next_departure_time: _,
          departure_time: _,
          from_stop: _,
         } => {
          let slow_trip = self.slow_trips.entry(trip_id).or_default();
          slow_trip.push(item);
        },
        QueueItemVariant::Connection { 
          trip_id, 
          route: _,
          departure_time: _,
          from_stop: _,
         } => {
          let slow_trip = self.slow_trips.entry(trip_id).or_default();
          slow_trip.push(item);
        },
        _ => () // late arrival by transfer - drop it
      }
      vec![] // the item will not be emitted
    }
  }

  /// finds all trips leaving a stop within a time period, includes the stop time for that stop and all following stops
  fn trips_from(&self, stop: StopId, period: Period) -> impl Iterator<Item = &TripStopRef> {
    let departures = self.data.get_departures_from(stop);
    departures.iter().filter(move |&stop_ref: &&TripStopRef| {
      // this is a slow lookup in a critical code section, if departure_time was part of the Ref this wouldn't be necessary
      let stop_time = self.data.stop_time(stop_ref);
      period.contains(stop_time.departure_time) && self.services.contains(&self.data.get_trip(&stop_time.trip_id).unwrap().service_id)
    })
  }

  /// finds all connections from a stop
  fn transfers_from(&self, stop: StopId) -> impl Iterator<Item = &Transfer> {
    self.data.get_transfers(&stop).map(|vec| vec.iter()).unwrap_or([].iter())
  }
}
