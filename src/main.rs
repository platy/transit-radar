use crate::gtfs::db::Suggester;
use std::error::Error;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use db::{GTFSSource, DayFilter};
use warp::Filter;
use urlencoding::decode;
use chrono::prelude::*;

mod arena;
mod gtfs;
use gtfs::*;
use gtfs::gtfstime::{Time, Period, Duration};

mod journey_graph;

use geo::algorithm::bearing::Bearing;

fn load_data(gtfs_dir: &Path, day_filter: DayFilter, time_period: Option<Period>) -> Result<db::GTFSData, Box<dyn Error>> {
    let source = &GTFSSource::new(gtfs_dir);

    let mut data;
    if let Some(data2) = source.load_cache(day_filter, time_period)? {
        data = data2
    } else {
        data = gtfs::db::GTFSData::new();
        data.load_transfers_of_stop(source)?;
        data.load_stops_by_id(source)?;
        data.load_trips_by_id(source, day_filter)?;
        data.load_routes_by_id(source)?;
        data.departure_lookup(time_period, &source)?;
        // source.write_cache(day_filter, time_period, &data)?;
    };
    Ok(data)
}

fn lookup<'r>(data: &'r db::GTFSData, station_name: String, period: Period) -> Result<FEData<'r>, db::SearchError> {
    let station = data.get_station_by_name(&station_name)?;
    let output = produce_tree_json(&data, station.stop_id, period);
    println!("Search for '{}' produced {} stations, {} trips and {} connections", station.stop_name, output.stops.len(), output.trips.len(), output.connections.len());
    Ok(output)
}

fn produce_tree_json<'r>(data: &'r db::GTFSData, station: StopId, period: Period) -> FEData<'r> {
    let mut plotter = journey_graph::JourneyGraphPlotter::new(period, data);
    let origin = data.get_stop(&station).unwrap();
    plotter.add_origin_station(origin);
    plotter.add_route_types(vec![
        // 2, // long distance rail
        3, // some kind of bus
        // 100, // Regional trains
        109, // SBahn
        400, // UBahn
        // 700 // Bus
        // 900 // Tram
    ]);

    let mut fe_stops: Vec<FEStop> = vec![];
    let mut fe_conns: Vec<FEConnection> = vec![];
    let mut fe_trips: HashMap<TripId, FERoute> = HashMap::new();
    let mut stop_id_to_idx = HashMap::new();
    let mut connections_check = HashSet::new();

    for item in plotter {
        match item {
            journey_graph::Item::Station {
                stop,
                earliest_arrival,
            } => {
                stop_id_to_idx.insert(stop.station_id(), fe_stops.len());
                fe_stops.push(FEStop {
                    bearing: origin.position().bearing(stop.position()),
                    name: stop.stop_name.replace(" (Berlin)", ""),
                    seconds: earliest_arrival - period.start(),
                });
            },
            journey_graph::Item::JourneySegment {
                departure_time, 
                arrival_time, 
                from_stop,
                to_stop,
            } => {
                let to = *stop_id_to_idx.get(&to_stop.station_id()).unwrap();
                let from_stop_or_station_id = from_stop.station_id();
                let from = *stop_id_to_idx.get(&from_stop_or_station_id).unwrap_or(&to);
                let kind = FEConnectionType::Connection;
                // only emit each connection once
                if connections_check.insert((from, to, None, kind)) {
                    fe_conns.push(FEConnection {
                        from,
                        to,
                        route_name: None,
                        from_seconds: departure_time - period.start(),
                        to_seconds: arrival_time - period.start(),
                    })
                }
            },
            journey_graph::Item::SegmentOfTrip {
                departure_time, 
                arrival_time, 
                from_stop,
                to_stop,
                trip_id,
                route_name,
                route_type,
            } => {
                let to = *stop_id_to_idx.get(&to_stop.station_id()).unwrap();
                let from_stop_or_station_id = from_stop.station_id();
                let from = *stop_id_to_idx.get(&from_stop_or_station_id).unwrap_or(&to);
                let kind = FEConnectionType::from(route_type);
                // only emit each connection once
                if connections_check.insert((from, to, Some(route_name), kind)) {
                    let route = fe_trips.entry(trip_id).or_insert(FERoute { route_name, kind, segments: vec![] });
                    route.segments.push(FESegment {
                        from,
                        to,
                        from_seconds: departure_time - period.start(),
                        to_seconds: arrival_time - period.start(),
                    })
                }
            },
            journey_graph::Item::ConnectionToTrip {
                departure_time, 
                arrival_time, 
                from_stop,
                to_stop,
                route_name,
            } => {
                let to = *stop_id_to_idx.get(&to_stop.station_id()).unwrap();
                let from_stop_or_station_id = from_stop.station_id();
                let from = *stop_id_to_idx.get(&from_stop_or_station_id).unwrap_or(&to);
                // only emit each connection once
                if connections_check.insert((from, to, Some(route_name), FEConnectionType::Connection)) {
                    fe_conns.push(FEConnection {
                        from,
                        to,
                        route_name: Some(route_name),
                        from_seconds: departure_time - period.start(),
                        to_seconds: arrival_time - period.start(),
                    })
                }
            },
        }
        
    }
    FEData {
        stops: fe_stops,
        connections: fe_conns,
        trips: fe_trips.into_iter().map(|(_k, v)| v).collect(),
        departure_day: "Saturday",
        departure_time: period.start(),
        duration_minutes: period.duration().mins(),
    }
}

use serde::Serialize;

#[derive(Serialize)]
struct FEData<'s> {
    stops: Vec<FEStop>,
    connections: Vec<FEConnection<'s>>,
    trips: Vec<FERoute<'s>>,
    departure_day: &'static str,
    departure_time: Time,
    duration_minutes: i32,
}

#[derive(Serialize)]
struct FEStop {
    bearing: f64,
    name: String,
    seconds: gtfstime::Duration,
}

#[derive(Serialize)]
struct FERoute<'s> {
    route_name: &'s str,
    kind: FEConnectionType,
    segments: Vec<FESegment>,
}

#[derive(Serialize)]
struct FESegment {
    from_seconds: gtfstime::Duration,
    to_seconds: gtfstime::Duration,
    from: usize,
    to: usize,
}

#[derive(Serialize)]
struct FEConnection<'s> {
    from_seconds: gtfstime::Duration,
    to_seconds: gtfstime::Duration,
    from: usize,
    to: usize,
    route_name: Option<&'s str>,
}

#[derive(Serialize, Eq, PartialEq, Hash, Copy, Clone)]
enum FEConnectionType {
    Connection, // walking, waiting
    Rail,//long distance 2
    Bus, //3
    RailwayService,//100 RE/RB
    SuburbanRailway, //SBahn 109
    UrbanRailwayService,//400
    BusService, //700
    TramService, //900
    Other(RouteType),
}

impl FEConnectionType {
    fn from(route_type: RouteType) -> FEConnectionType {
        use FEConnectionType::*;
        match route_type {
            2 => Rail,
            3 => Bus,
            100 => RailwayService,
            109 => SuburbanRailway,
            400 => UrbanRailwayService,
            700 => BusService,
            900 => TramService,
            other => Other(other),
        }
    }
}

fn with_data<D: Sync + Send>(db: Arc<D>) -> impl Filter<Extract = (Arc<D>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || db.clone())
}

async fn json_tree_handler(name: String, data: Arc<db::GTFSData>) -> Result<impl warp::Reply, warp::Rejection> {
    let date_time = chrono::Utc::now().with_timezone(&chrono_tz::Europe::Berlin);
    let now = Time::from_hms(date_time.hour(), date_time.minute(), date_time.second());
    let period = Period::between(now, now + Duration::minutes(30));

    match decode(&name) {
        Ok(name) => 
            match lookup(&data, name, period) {
                Ok(result) => Ok(warp::reply::json(&result)),
                Err(error) => Err(warp::reject::custom(error)),
            },
        Err(err) => {
            eprintln!("dir: failed to decode route={:?}: {:?}", name, err);
            return Err(warp::reject::reject());
        }
    }
}

#[derive(Serialize)]
struct FEStationLookup<'s> {
    stop_id: StopId,
    name: &'s str,
}

async fn station_search_handler(query: String, data: Arc<db::GTFSData>, station_search: Arc<Suggester<StopId>>) -> Result<impl warp::Reply, warp::Rejection> {
    match decode(&query) {
        Ok(query) => {
            let mut result = Vec::new();
            let mut count = 0;
            for stop_id in station_search.search(&query) {
                if count > 20 {
                    break;
                }
                let stop = data.get_stop(&stop_id).expect("to find stop referenced by search");
                result.push(FEStationLookup {
                    stop_id: stop_id,
                    name: &stop.stop_name,
                });
                count += 1;
            }
            Ok(warp::reply::json(&result))
        },
        Err(err) => {
            eprintln!("dir: failed to decode query={:?}: {:?}", &query, err);
            return Err(warp::reject::not_found());
        }
    }
}

fn json_tree_route(data: Arc<db::GTFSData>) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let cors = warp::cors()
        .allow_any_origin();
    warp::path!("from" / String)
        .and(with_data(data))
        .and_then(json_tree_handler)
        .with(cors)
}

fn station_name_search_route(data: Arc<db::GTFSData>, station_search: Arc<Suggester<StopId>>) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let cors = warp::cors()
        .allow_any_origin();
    warp::path!("searchStation" / String)
        .and(with_data(data))
        .and(with_data(station_search))
        .and_then(station_search_handler)
        .with(cors)
}

#[tokio::main]
async fn main() {
    let port = std::env::var("PORT").unwrap_or("8080".to_owned()).parse().unwrap();
    let static_dir = std::env::var("STATIC_DIR").unwrap_or("frontend/build".to_owned());
    let gtfs_dir = std::env::var("GTFS_DIR").unwrap_or("gtfs".to_owned());
    let gtfs_dir = Path::new(&gtfs_dir);

    let data = Arc::new(load_data(
        &gtfs_dir,
        DayFilter::Saturday, 
        None,
    ).unwrap());
    let station_name_index = Arc::new(data.build_station_word_index());

    eprintln!("Starting web server on port {}", port);
    warp::serve(warp::fs::dir(static_dir)
            .or(json_tree_route(data.clone()))
            .or(station_name_search_route(data.clone(), station_name_index))
        )
        .run(([127, 0, 0, 1], port))
        .await;
}
