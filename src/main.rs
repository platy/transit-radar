use crate::gtfs::db::Suggester;
use std::error::Error;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use db::{GTFSSource, DayFilter};
use warp::Filter;
use urlencoding::decode;

mod arena;
mod gtfs;
use gtfs::*;
use gtfs::gtfstime::{Time, Period};

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
        data.load_trips_by_id(source)?;
        data.load_routes_by_id(source)?;
        data.departure_lookup(day_filter, time_period, &source)?;
        source.write_cache(day_filter, time_period, &data)?;
    };
    Ok(data)
}

fn lookup<'r>(data: &'r db::GTFSData, station_name: String, period: Period) -> Result<FEData<'r>, db::SearchError> {
    let station = data.get_station_by_name(&station_name)?;
    let output = produce_tree_json(&data, station.stop_id, period);
    println!("Search for '{}' produced {} stations and {} connections", station.stop_name, output.stops.len(), output.connections.len());
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
                route_name,
                route_type,
            } => {
                let to = *stop_id_to_idx.get(&to_stop.station_id()).unwrap();
                let from_stop_or_station_id = from_stop.station_id();
                let from = *stop_id_to_idx.get(&from_stop_or_station_id).unwrap_or(&to);
                let kind = FEConnectionType::from(route_type);
                // only emit each connection once
                if connections_check.insert((from, to, route_name, kind)) {
                    fe_conns.push(FEConnection {
                        from,
                        to,
                        route_name,
                        kind,
                        from_seconds: departure_time - period.start(),
                        to_seconds: arrival_time - period.start(),
                    })
                }
            }
        }
        
    }
    FEData {
        stops: fe_stops,
        connections: fe_conns,
    }
}

use serde::Serialize;

#[derive(Serialize)]
struct FEData<'s> {
    stops: Vec<FEStop>,
    connections: Vec<FEConnection<'s>>,
}

#[derive(Serialize)]
struct FEStop {
    bearing: f64,
    name: String,
    seconds: gtfstime::Duration,
}

#[derive(Serialize)]
struct FEConnection<'s> {
    from_seconds: gtfstime::Duration,
    to_seconds: gtfstime::Duration,
    from: usize,
    to: usize,
    route_name: Option<&'s str>,
    kind: FEConnectionType,
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
    fn from(route_type: Option<RouteType>) -> FEConnectionType {
        use FEConnectionType::*;
        if let Some(route_type) = route_type {
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
        } else {
            Connection
        }
    }
}



fn with_data<D: Sync + Send>(db: Arc<D>) -> impl Filter<Extract = (Arc<D>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || db.clone())
}


async fn json_tree_handler(name: String, data: Arc<db::GTFSData>) -> Result<impl warp::Reply, warp::Rejection> {
    let period = Period::between(Time::parse("19:00:00").unwrap(), Time::parse("19:30:00").unwrap());

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
        DayFilter::Friday, 
        Some(Period::between(Time::parse("19:00:00").unwrap(), Time::parse("19:30:00").unwrap())),
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
