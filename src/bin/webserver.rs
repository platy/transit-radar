use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use warp::Filter;
use urlencoding::decode;
use chrono::prelude::*;

use transit_radar::journey_graph;
use transit_radar::gtfs::*;
use transit_radar::gtfs::db;
use transit_radar::Suggester;

use geo::algorithm::bearing::Bearing;

fn lookup<'r>(data: &'r db::GTFSData, station_name: String, options: RadarOptions, day: Day, period: Period) -> Result<FEData<'r>, db::SearchError> {
    let station = data.get_station_by_name(&station_name)?;
    let output = produce_tree_json(&data, station.stop_id, day, period, &options);
    println!("Search for '{}' {:?} produced {} stations, {} trips and {} connections", station.stop_name, options, output.stops.len(), output.trips.len(), output.connections.len());
    Ok(output)
}

fn produce_tree_json<'r>(data: &'r db::GTFSData, station: StopId, day: Day, period: Period, options: &RadarOptions) -> FEData<'r> {
    let mut plotter = journey_graph::JourneyGraphPlotter::new(day, period, data);
    let origin = data.get_stop(&station).unwrap();
    plotter.add_origin_station(origin);
    if options.ubahn { plotter.add_route_type(RouteType::UrbanRailway); }
    if options.sbahn { plotter.add_route_type(RouteType::SuburbanRailway); }
    if options.bus { plotter.add_route_type(RouteType::BusService); }
    if options.tram { plotter.add_route_type(RouteType::TramService); }
    if options.regio { plotter.add_route_type(RouteType::RailwayService); }
    if options.bus { plotter.add_route_type(RouteType::Bus); }

    let mut fe_stops: Vec<FEStop> = vec![];
    let mut fe_conns: Vec<FEConnection> = vec![];
    let mut fe_trips: HashMap<TripId, FERoute> = HashMap::new();
    let mut stop_id_to_idx = HashMap::new();

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
                fe_conns.push(FEConnection {
                    from,
                    to,
                    route_name: None,
                    kind: None,
                    from_seconds: departure_time - period.start(),
                    to_seconds: arrival_time - period.start(),
                })
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
                let trip = fe_trips.entry(trip_id).or_insert(FERoute { route_name, kind, segments: vec![] });
                trip.segments.push(FESegment {
                    from,
                    to,
                    from_seconds: departure_time - period.start(),
                    to_seconds: arrival_time - period.start(),
                });
            },
            journey_graph::Item::ConnectionToTrip {
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
                fe_conns.push(FEConnection {
                    from,
                    to,
                    route_name: Some(route_name),
                    kind: Some(FEConnectionType::from(route_type)),
                    from_seconds: departure_time - period.start(),
                    to_seconds: arrival_time - period.start(),
                })
            },
        }
    }
    FEData {
        stops: fe_stops,
        connections: fe_conns,
        trips: fe_trips.into_iter().map(|(_k, v)| v).collect(),
        timetable_date: data.timetable_start_date().to_string(),
        departure_day: day.to_string(),
        departure_time: period.start(),
        duration_minutes: period.duration().to_mins(),
    }
}

use serde::Serialize;

#[derive(Serialize)]
struct FEData<'s> {
    stops: Vec<FEStop>,
    connections: Vec<FEConnection<'s>>,
    trips: Vec<FERoute<'s>>,
    timetable_date: String,
    departure_day: String,
    departure_time: Time,
    duration_minutes: i32,
}

#[derive(Serialize)]
struct FEStop {
    bearing: f64,
    name: String,
    seconds: Duration,
}

#[derive(Serialize)]
struct FERoute<'s> {
    route_name: &'s str,
    kind: FEConnectionType,
    segments: Vec<FESegment>,
}

#[derive(Serialize)]
struct FESegment {
    from_seconds: Duration,
    to_seconds: Duration,
    from: usize,
    to: usize,
}

#[derive(Serialize)]
struct FEConnection<'s> {
    from_seconds: Duration,
    to_seconds: Duration,
    from: usize,
    to: usize,
    route_name: Option<&'s str>,
    kind: Option<FEConnectionType>,
}

#[derive(Serialize, Eq, PartialEq, Hash, Copy, Clone)]
enum FEConnectionType {
    Rail,//long distance 2
    Bus, //3
    RailwayService,//100 RE/RB
    SuburbanRailway, //SBahn 109
    UrbanRailwayService,//400
    BusService, //700
    TramService, //900
    WaterTransportService, //1000
}

impl FEConnectionType {
    fn from(route_type: RouteType) -> FEConnectionType {
        use FEConnectionType::*;
        match route_type {
            RouteType::Rail => Rail,
            RouteType::Bus => Bus,
            RouteType::RailwayService => RailwayService,
            RouteType::SuburbanRailway => SuburbanRailway,
            RouteType::UrbanRailway => UrbanRailwayService,
            RouteType::BusService => BusService,
            RouteType::TramService => TramService,
            RouteType::WaterTransportService => WaterTransportService,
        }
    }
}

fn with_data<D: Sync + Send>(db: Arc<D>) -> impl Filter<Extract = (Arc<D>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || db.clone())
}

async fn json_tree_handler(name: String, options: RadarOptions, data: Arc<db::GTFSData>) -> Result<impl warp::Reply, warp::Rejection> {
    let date_time = chrono::Utc::now().with_timezone(&chrono_tz::Europe::Berlin);
    let now = Time::from_hms(date_time.hour(), date_time.minute(), date_time.second());
    let day = match date_time.weekday() {
        Weekday::Mon => Day::Monday,
        Weekday::Tue => Day::Tuesday,
        Weekday::Wed => Day::Wednesday,
        Weekday::Thu => Day::Thursday,
        Weekday::Fri => Day::Friday,
        Weekday::Sat => Day::Saturday,
        Weekday::Sun => Day::Sunday,
    };
    let period = Period::between(now, now + Duration::minutes(30));

    match decode(&name) {
        Ok(name) => 
            match lookup(&data, name, options, day, period) {
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
    stop_id: u64,
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
                    stop_id: stop_id.into_inner(),
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

#[derive(Debug, serde::Deserialize)]
pub struct RadarOptions {
    pub ubahn: bool,
    pub sbahn: bool,
    pub bus: bool,
    pub regio: bool,
    pub tram: bool,
}

fn json_tree_route(data: Arc<db::GTFSData>) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let cors = warp::cors()
        .allow_any_origin();
    warp::path!("from" / String)
        .and(warp::query::<RadarOptions>())
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

    let data = Arc::new(db::GTFSData::load_data(
        &gtfs_dir,
        db::DayFilter::All, 
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
