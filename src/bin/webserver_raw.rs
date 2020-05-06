use chrono::prelude::*;
use futures::future;
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
};
use urlencoding::decode;
use warp::{reject, Filter};

use radar_search::{journey_graph, search_data::*, time::*};
use transit_radar::gtfs::db;

struct SessionContainer<S> {
    map: Mutex<HashMap<u64, Arc<Mutex<S>>>>,
    next_session_id: AtomicU64,
}

#[derive(serde::Deserialize)]
struct SessionKey {
    id: Option<u64>,
    count: Option<u32>,
}

#[derive(Debug)]
struct SessionOutOfSync;

impl reject::Reject for SessionOutOfSync {}

impl<S: From<u64>> SessionContainer<S> {
    fn new() -> SessionContainer<S> {
        SessionContainer {
            map: Mutex::new(HashMap::new()),
            next_session_id: AtomicU64::new(1000),
        }
    }

    pub fn session_filter(
        &self,
        key: SessionKey,
    ) -> Result<(String, Arc<Mutex<S>>), reject::Rejection> {
        let mut map = self.map.lock().unwrap();
        let session_id = key.id.unwrap_or_else(|| self.new_session_id());
        let update_number = key.count.unwrap_or(0);
        let session = map
            .entry(session_id)
            .or_insert_with(|| Arc::new(Mutex::new(From::from(session_id))));
        // if (*session.lock().unwrap()).update_number == update_number {
        Ok((session_id.to_string(), session.clone()))
        // } else {
        //     Err(reject::custom(SessionOutOfSync))
        // }
    }

    fn new_session_id(&self) -> u64 {
        self.next_session_id.fetch_add(1, Ordering::SeqCst)
    }
}

fn with_session<S: Sync + Send + From<u64>>(
) -> impl Filter<Extract = ((String, Arc<Mutex<S>>),), Error = reject::Rejection> + Clone {
    let container = Arc::new(SessionContainer::new());
    warp::query::<SessionKey>()
        .and_then(move |header| future::ready(container.session_filter(header)))
}

fn with_data<D: Sync + Send>(
    db: Arc<D>,
) -> impl Filter<Extract = (Arc<D>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || db.clone())
}

fn day_time(date_time: chrono::DateTime<Utc>) -> (Day, Time) {
    let date_time = date_time.with_timezone(&chrono_tz::Europe::Berlin);
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
    (day, now)
}

fn filter_data<'r>(
    data: &'r GTFSData,
    station_name: String,
    options: RadarOptions,
    day: Day,
    period: Period,
) -> Result<GTFSData, db::SearchError> {
    let station = db::get_station_by_name(data, &station_name)?;
    let mut plotter = journey_graph::JourneyGraphPlotter::new(day, period, data);
    let origin = data.get_stop(&station.stop_id).unwrap();
    plotter.add_origin_station(origin);
    if options.ubahn {
        plotter.add_route_type(RouteType::UrbanRailway);
    }
    if options.sbahn {
        plotter.add_route_type(RouteType::SuburbanRailway);
    }
    if options.bus {
        plotter.add_route_type(RouteType::BusService);
    }
    if options.tram {
        plotter.add_route_type(RouteType::TramService);
    }
    if options.regio {
        plotter.add_route_type(RouteType::RailwayService);
    }
    if options.bus {
        plotter.add_route_type(RouteType::Bus);
    }
    Ok(plotter.filtered_data())
}

async fn filtered_data_handler(
    name: String,
    options: RadarOptions,
    data: Arc<GTFSData>,
    (session_id, session): (String, Arc<Mutex<GTFSDataSession>>),
) -> Result<impl warp::Reply, warp::Rejection> {
    let (day, now) = day_time(chrono::Utc::now());
    let period = Period::between(now, now + Duration::minutes(30));

    match decode(&name) {
        Ok(name) => {
            let data =
                filter_data(&data, name, options, day, period).map_err(warp::reject::custom)?;
            match session.lock() {
                Ok(mut session) => {
                    let mut buf = Vec::<u8>::new();
                    let mut serializer = rmp_serde::Serializer::new(&mut buf)
                        .with_struct_tuple()
                        .with_integer_variants();

                    (*session)
                        .add_data(data)
                        .serialize(&mut serializer)
                        .map_err(|err| {
                            eprintln!("failed to serialize data {:?}", err);
                            warp::reject::reject()
                        })?;
                    Ok(buf)
                }
                Err(lock_error) => {
                    eprintln!("session corrupted new session : {:?}", lock_error);
                    Err(warp::reject::reject())
                }
            }
        }
        Err(err) => {
            eprintln!("failed to decode route={:?}: {:?}", name, err);
            return Err(warp::reject::reject());
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

fn filtered_data_route(
    data: Arc<GTFSData>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let cors = warp::cors().allow_any_origin();
    warp::path!("data" / String)
        .and(warp::query::<RadarOptions>())
        .and(with_data(data))
        .and(with_session())
        .and_then(filtered_data_handler)
        .with(cors)
}

#[tokio::main]
async fn main() {
    let port = std::env::var("PORT")
        .unwrap_or("8080".to_owned())
        .parse()
        .unwrap();
    let static_dir = std::env::var("STATIC_DIR").unwrap_or("seed-quickstart".to_owned());
    let gtfs_dir = std::env::var("GTFS_DIR").unwrap_or("gtfs".to_owned());
    let gtfs_dir = Path::new(&gtfs_dir);

    let colors = db::load_colors(Path::new("./Linienfarben.csv")).unwrap();
    let data = Arc::new(db::load_data(&gtfs_dir, db::DayFilter::All, colors).unwrap());

    eprintln!("Starting web server on port {}", port);
    warp::serve(warp::fs::dir(static_dir).or(filtered_data_route(data.clone())))
        .run(([127, 0, 0, 1], port))
        .await;
}
