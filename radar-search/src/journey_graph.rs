use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};
use std::fmt;
use std::iter::FromIterator;

use crate::search_data::{
    Day, GTFSData, RequiredData, Route, RouteType, ServiceId, Stop, StopId, TripId,
};
use crate::time::{Period, Time};

/// Runs an algoritm to build a tree of all fastest journeys from a start point
pub struct Plotter<'r> {
    period: Period, // Search of journeys is within this period
    route_types: HashSet<RouteType>,
    data: &'r GTFSData,
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
}

/// Output of the algorithm, Items are produced in order of arrival time
impl<'r> Iterator for Plotter<'r> {
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

impl<'r> Plotter<'r> {
    pub fn new(day: Day, period: Period, data: &'r GTFSData) -> Plotter<'r> {
        Plotter {
            period,
            services: data.services_of_day(day),
            queue: BinaryHeap::new(),
            catch_up: VecDeque::new(),
            enqueued_trips: HashSet::new(),
            slow_trips: HashMap::new(),
            stops: HashMap::new(),
            emitted_stations: HashSet::new(),
            data,
            route_types: HashSet::new(),
        }
    }

    /// Add an origin station to start the search from
    pub fn add_origin_station(&mut self, origin: &'r Stop) {
        self.queue.push(QueueItem {
            arrival_time: self.period.start(),
            to_stop: origin,
            variant: QueueItemVariant::OriginStation,
        });
    }

    /// Add a route type to be searched
    pub fn add_route_type(&mut self, route_type: RouteType) {
        self.route_types.insert(route_type);
    }

    /// Performs the whole search, producing a filtered search data object with only the stops and trips needed for the search
    pub fn filtered_data(mut self) -> RequiredData {
        let mut builder = self.data.build_from();
        loop {
            let items = self.next_block_raw();
            if items.is_empty() {
                break;
            } else {
                for QueueItem {
                    arrival_time: _,
                    to_stop,
                    variant,
                } in items
                {
                    builder.keep_stop(to_stop.stop_id);
                    if let Some(parent_id) = to_stop.parent_station() {
                        builder.keep_stop(parent_id);
                    }
                    if let Some(trip_id) = variant.get_trip_id() {
                        builder.keep_trip(trip_id);
                    }
                }
            }
        }
        builder.build()
    }

    /// returns the next items to be emitted in order, or empty if there are no more and the process halts
    fn next_block(&mut self) -> Vec<Item<'r>> {
        self.next_block_raw()
            .into_iter()
            .flat_map(|item| {
                let mut to_emit = vec![];
                // if this arrives at a new station, emit that first
                if self.emitted_stations.insert(item.to_stop.station_id()) {
                    to_emit.push(Item::Station {
                        stop: item.to_stop,
                        earliest_arrival: item.arrival_time,
                        name_trunk_length: if item.variant.is_stop_on_trip() {
                            item.variant
                                .get_from_stop()
                                .and_then(|from_stop| {
                                    item.to_stop
                                        .short_stop_name
                                        .starts_with(&from_stop.short_stop_name)
                                        .then_some(from_stop.short_stop_name.len())
                                })
                                .unwrap_or_default()
                        } else {
                            0
                        },
                    });
                }
                if let Some(item) = self.convert_item(item) {
                    to_emit.push(item);
                }
                to_emit // we found something that's worth drawing
            })
            .collect()
    }

    /// returns the next processed items in order, or empty if there are no more and the process halts
    fn next_block_raw(&mut self) -> Vec<QueueItem<'r>> {
        while let Some(item) = self.queue.pop() {
            if self.period.contains(item.arrival_time) {
                let processed: Vec<QueueItem<'r>> = self.process_queue_item(item);
                if !processed.is_empty() {
                    return processed;
                }
            } else {
                return vec![]; // we ran out of the time period
            }
        }
        vec![]
    }

    /// Produces an output item for a queue item
    fn convert_item(
        &self,
        QueueItem {
            to_stop,
            mut arrival_time,
            variant,
        }: QueueItem<'r>,
    ) -> Option<Item<'r>> {
        match variant {
            QueueItemVariant::OriginStation => None,
            QueueItemVariant::Transfer {
                from_stop,
                departure_time,
            } => Some(Item::Transfer {
                from_stop,
                to_stop,
                departure_time,
                arrival_time,
            }),
            QueueItemVariant::Connection {
                trip_id,
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
                route_color: &route.route_color,
                trip_id,
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
                    route_color: &route.route_color,
                })
            }
        }
    }

    fn enqueue_transfers_from_stop(&mut self, stop: &'r Stop, departure_time: Time) {
        let mut to_add = vec![];
        for transfer in &stop.transfers {
            if !self.stops.contains_key(&transfer.to_stop_id) {
                if let Some(to_stop) = self.data.get_stop(transfer.to_stop_id) {
                    to_add.push(QueueItem {
                        to_stop,
                        arrival_time: departure_time
                            + transfer
                                .min_transfer_time
                                .unwrap_or_else(chrono::Duration::zero),
                        variant: QueueItemVariant::Transfer {
                            from_stop: stop,
                            departure_time,
                        },
                    });
                }
            }
        }
        self.queue.extend(to_add);
    }

    fn enqueue_transfers_from_station(&mut self, station: &'r Stop, departure_time: Time) {
        let mut to_add = vec![];
        for transfer in &station.transfers {
            if !self.stops.contains_key(&transfer.to_stop_id) {
                // parent stations transfer to parents, so transfer to the children as well (but aybe they hav entries in transfer to use without this implicit transfer?)
                // we ignore any missing stops in case this is a partial data set
                let to_stop = self.data.get_stop(transfer.to_stop_id);
                let iter = to_stop
                    .iter()
                    .map(|stop| &stop.stop_id)
                    .chain(to_stop.iter().flat_map(|stop| stop.children()));
                for &to_stop_id in iter {
                    if let Some(to_stop) = self.data.get_stop(to_stop_id) {
                        to_add.push(QueueItem {
                            to_stop,
                            arrival_time: departure_time
                                + transfer
                                    .min_transfer_time
                                    .unwrap_or_else(chrono::Duration::zero),
                            variant: QueueItemVariant::Transfer {
                                from_stop: station,
                                departure_time,
                            },
                        });
                    }
                }
            }
        }
        self.queue.extend(to_add);
    }

    fn enqueue_immediate_transfers_to_children_of(&mut self, stop: &'r Stop, arrival_time: Time) {
        let to_stop = self
            .data
            .get_stop(stop.stop_id)
            .expect("Origin stop to exist");
        let origin_stops = Some(&to_stop.stop_id).into_iter().chain(to_stop.children());
        let to_add: Vec<QueueItem> = origin_stops
            .filter_map(|&stop_id| {
                self.data.get_stop(stop_id).map(|child_stop|
                    // immediately transfer to all the stops of this origin station
                    QueueItem {
                        to_stop: child_stop,
                        arrival_time,
                        variant: QueueItemVariant::Transfer {
                            from_stop: stop,
                            departure_time: arrival_time,
                        },
                    })
            })
            .collect();
        self.queue.extend(to_add);
    }

    fn enqueue_connections_and_trips(
        &mut self,
        item: &QueueItem<'r>,
        from_stop: &'r Stop,
        departure_time: Time,
    ) -> bool {
        let mut to_add = vec![];
        for (trip, stops) in self.data.trips_from(
            item.to_stop,
            &self.services,
            self.period.with_start(item.arrival_time),
        ) {
            let stops = Vec::from_iter(stops);
            let trip_id = trip.trip_id;
            let mut trip_to_add = vec![];
            // check that route type is allowed
            let route = &trip.route;
            if self.route_types.contains(&route.route_type) {
                // enqueue connection (transfer + wait)
                trip_to_add.push(QueueItem {
                    to_stop: item.to_stop,
                    arrival_time: stops[0].departure_time,
                    variant: QueueItemVariant::Connection {
                        trip_id,
                        route,
                        from_stop,
                        departure_time,
                    },
                });
                for window in stops.windows(2) {
                    if let [from_stop, to_stop] = window {
                        if self.period.contains(to_stop.arrival_time) {
                            // these stops wont be there if this stoptime is going to be filtered out later anyway
                            if let (Some(to_stop_stop), Some(from_stop_stop)) = (
                                self.data.get_stop(to_stop.stop_id),
                                self.data.get_stop(from_stop.stop_id),
                            ) {
                                trip_to_add.push(QueueItem {
                                    to_stop: to_stop_stop,
                                    arrival_time: to_stop.arrival_time,
                                    variant: QueueItemVariant::StopOnTrip {
                                        trip_id,
                                        route,
                                        previous_arrival_time: from_stop.arrival_time,
                                        next_departure_time: to_stop.departure_time,
                                        from_stop: from_stop_stop,
                                        departure_time: from_stop.departure_time,
                                    },
                                });
                            }
                        }
                    } else {
                        panic!("Bad window");
                    }
                }
                to_add.push((trip_id, trip_to_add));
            }
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
        self.stops.get(&stop_id).cloned()
    }

    fn filter_slow_trip(&mut self, slow_trip: Vec<QueueItem<'r>>) -> Vec<QueueItem<'r>> {
        // this trip became useful but it might be that we don't board at the first stop where we encountered it, we should board at the stop we can get to the earliest, not the earliest we can board this trip
        let boarding_opportunities = slow_trip.iter().enumerate().filter_map(|(i, item)| {
            // Each item must only be a StopOnTrip or a Connection
            let from_stop = item.variant.get_from_stop().expect(
                "A slow trip must only contain connections and stops, no transfers or origins",
            );
            self.earliest_arrival_at(from_stop.stop_id)
                .map(|time| (i, time, item))
        });
        // index of the stop on this trip that we arrive at first
        if let Some((boarding_idx, first_arrival, item)) =
            boarding_opportunities.min_by_key(|(_i, first_arrival, _item)| *first_arrival)
        {
            if boarding_idx > 0 {
                if let QueueItemVariant::StopOnTrip {
                    from_stop,
                    departure_time,
                    trip_id,
                    route,
                    previous_arrival_time: _,
                    next_departure_time: _,
                } = item.variant
                {
                    // we board later and so need a new connection for that
                    let connection = QueueItem {
                        arrival_time: departure_time,
                        to_stop: from_stop,
                        variant: QueueItemVariant::Connection {
                            from_stop,
                            departure_time: first_arrival,
                            trip_id,
                            route,
                        },
                    };
                    Some(connection)
                        .into_iter()
                        .chain(slow_trip.into_iter().skip(boarding_idx))
                        .collect()
                } else {
                    panic!("expected {:?} to be a StopOnTrip", item);
                }
            } else {
                slow_trip
            }
        } else {
            slow_trip
        }
    }

    fn set_arrival_time(&mut self, stop_id: StopId, new_arrival_time: Time) -> bool {
        let new_arrival_is_earlier = self
            .stops
            .get(&stop_id)
            .map_or(true, |&previous_earliest_arrival| {
                new_arrival_time < previous_earliest_arrival
            });
        if new_arrival_is_earlier {
            self.stops.insert(stop_id, new_arrival_time);
            true
        } else {
            false
        }
    }

    /// Processes the item, enqueuing any following segments and possibly returning the processed items to be converted and emitted
    fn process_queue_item(&mut self, item: QueueItem<'r>) -> Vec<QueueItem<'r>> {
        if self.set_arrival_time(item.to_stop.stop_id, item.arrival_time) {
            // if this changes the earliest arrival time for this stop, we possibly have new connections / trips
            match item.variant {
                QueueItemVariant::StopOnTrip {
                    trip_id,
                    route: _route,
                    previous_arrival_time: _,
                    next_departure_time: _,
                    from_stop: _,
                    departure_time: _,
                } => {
                    if !item.to_stop.is_station() {
                        self.enqueue_transfers_from_stop(item.to_stop, item.arrival_time);
                    }
                    if let Some(to_station) = self.data.get_stop(item.to_stop.station_id()) {
                        self.enqueue_transfers_from_station(to_station, item.arrival_time);
                    }
                    // only emit if we got to a new station
                    if self.emitted_stations.contains(&item.to_stop.station_id()) {
                        let slow_trip = self.slow_trips.entry(trip_id).or_default();
                        slow_trip.push(item);
                        vec![]
                    } else {
                        // if this now made some slow stops on the trip relevant, they should be emitted as well
                        let slow_trip = self.slow_trips.remove(&trip_id);
                        if let Some(slow_trip) = slow_trip {
                            let mut to_emit = self.filter_slow_trip(slow_trip);
                            to_emit.push(item);
                            to_emit
                        } else {
                            vec![item]
                        }
                    }
                }
                QueueItemVariant::Connection {
                    trip_id: _,
                    route: _,
                    from_stop: _,
                    departure_time: _,
                } => {
                    panic!("Unexpected");
                }
                QueueItemVariant::Transfer {
                    from_stop,
                    departure_time,
                } => {
                    let extended =
                        self.enqueue_connections_and_trips(&item, from_stop, departure_time);
                    // we don't emit transfers unless they are to a new station which accesses other trips
                    if !extended || from_stop.station_id() == item.to_stop.station_id() {
                        vec![]
                    } else {
                        vec![item]
                    }
                }
                QueueItemVariant::OriginStation => {
                    self.enqueue_immediate_transfers_to_children_of(
                        item.to_stop,
                        item.arrival_time,
                    );
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
                }
                | QueueItemVariant::Connection {
                    trip_id,
                    route: _,
                    departure_time: _,
                    from_stop: _,
                } => {
                    let slow_trip = self.slow_trips.entry(trip_id).or_default();
                    slow_trip.push(item);
                }
                _ => (), // late arrival by transfer - drop it
            }
            vec![] // the item will not be emitted
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
        match self.variant {
            QueueItemVariant::OriginStation => f
                .debug_struct("Origin")
                .field("stop", self.to_stop)
                .field("time", &self.arrival_time)
                .finish(),
            QueueItemVariant::Transfer {
                from_stop,
                departure_time,
            } => f
                .debug_struct("Transfer")
                .field("from", &from_stop)
                .field("departing", &departure_time)
                .field("to_stop", self.to_stop)
                .field("arrival_time", &self.arrival_time)
                .finish(),
            QueueItemVariant::Connection {
                trip_id,
                route,
                from_stop,
                departure_time,
            } => f
                .debug_struct("Connection")
                .field("to_route", &route)
                .field("trip_id", &trip_id)
                .field("from", from_stop)
                .field("departing", &departure_time)
                .field("to_stop", self.to_stop)
                .field("arrival_time", &self.arrival_time)
                .finish(),
            QueueItemVariant::StopOnTrip {
                trip_id,
                route,
                previous_arrival_time: _,
                next_departure_time: _,
                from_stop,
                departure_time,
            } => f
                .debug_struct("StopOnTrip")
                .field("route", &route)
                .field("trip_id", &trip_id)
                .field("from", from_stop)
                .field("departing", &departure_time)
                .field("to_stop", self.to_stop)
                .field("arrival_time", &self.arrival_time)
                .finish(),
        }
    }
}

/// The ordering on the queue items puts those with the earliest arrival times as the greatest,
/// so that they will be highest priority in the `BinaryHeap`, then (as an occasional bus route has sub-minute arrival times), it does the same thong each with previous arrival time, departure time and next departure time in the case of a stop. Then all the other fields need to be
/// taken into account for a full ordering
impl<'r> Ord for QueueItem<'r> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.arrival_time
            .cmp(&other.arrival_time)
            .reverse()
            .then_with(|| match (&self.variant, &other.variant) {
                (
                    QueueItemVariant::StopOnTrip {
                        departure_time: d1,
                        previous_arrival_time: p1,
                        next_departure_time: n1,
                        ..
                    },
                    QueueItemVariant::StopOnTrip {
                        departure_time: d2,
                        previous_arrival_time: p2,
                        next_departure_time: n2,
                        ..
                    },
                ) => p1
                    .cmp(p2)
                    .then_with(|| d1.cmp(d2))
                    .then_with(|| n1.cmp(n2))
                    .reverse(),
                _ => Ordering::Equal,
            })
            .then_with(|| self.variant.cmp(&other.variant))
            .then_with(|| self.to_stop.cmp(other.to_stop))
    }
}

impl<'r> PartialOrd for QueueItem<'r> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<'r> PartialEq for QueueItem<'r> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl<'r> Eq for QueueItem<'r> {}

#[derive(Debug, Ord, PartialOrd, PartialEq, Eq)]
enum QueueItemVariant<'r> {
    StopOnTrip {
        departure_time: Time,
        from_stop: &'r Stop,
        trip_id: TripId,
        route: &'r Route,
        previous_arrival_time: Time, // arrival at the from stop
        next_departure_time: Time,   // departure from the to stop
    },
    Connection {
        departure_time: Time,
        from_stop: &'r Stop,
        trip_id: TripId,
        route: &'r Route,
    },
    Transfer {
        departure_time: Time,
        from_stop: &'r Stop,
    },
    OriginStation,
}

impl<'r> QueueItemVariant<'r> {
    fn is_stop_on_trip(&self) -> bool {
        matches!(self, QueueItemVariant::StopOnTrip { .. })
    }

    fn get_from_stop(&self) -> Option<&'r Stop> {
        match self {
            QueueItemVariant::Connection {
                departure_time: _,
                from_stop,
                trip_id: _,
                route: _,
            }
            | QueueItemVariant::StopOnTrip {
                departure_time: _,
                from_stop,
                trip_id: _,
                route: _,
                previous_arrival_time: _,
                next_departure_time: _,
            }
            | QueueItemVariant::Transfer {
                departure_time: _,
                from_stop,
            } => Some(from_stop),
            QueueItemVariant::OriginStation => None,
        }
    }

    fn get_trip_id(&self) -> Option<TripId> {
        match self {
            QueueItemVariant::Connection {
                departure_time: _,
                from_stop: _,
                trip_id,
                route: _,
            }
            | QueueItemVariant::StopOnTrip {
                departure_time: _,
                from_stop: _,
                trip_id,
                route: _,
                previous_arrival_time: _,
                next_departure_time: _,
            } => Some(*trip_id),
            QueueItemVariant::Transfer {
                departure_time: _,
                from_stop: _,
            }
            | QueueItemVariant::OriginStation => None,
        }
    }
}

#[derive(Debug)]
pub enum Item<'r> {
    Transfer {
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
        route_color: &'r str,
        trip_id: TripId,
    },
    SegmentOfTrip {
        departure_time: Time,
        arrival_time: Time,
        from_stop: &'r Stop,
        to_stop: &'r Stop,
        trip_id: TripId,
        route_name: &'r str,
        route_type: RouteType,
        route_color: &'r str,
    },
    Station {
        stop: &'r Stop,
        earliest_arrival: Time,
        name_trunk_length: usize,
    },
}
