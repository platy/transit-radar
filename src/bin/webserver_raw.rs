use chrono::prelude::*;
use serde::Serialize;
use std::path::Path;
use std::sync::{Arc, Mutex};
use urlencoding::decode;
use warp::Filter;

use radar_search::{journey_graph, search_data::*, search_data_sync::*, time::*};
use transit_radar::gtfs::db;

mod endpoints;
mod web_util;
use web_util::*;

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

fn filter_data(
    data: &GTFSData,
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
    session: Arc<Mutex<GTFSDataSession>>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let (day, _now) = day_time(chrono::Utc::now());
    let period = Period::between(options.start_time, options.end_time);

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

                    session
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
            Err(warp::reject::reject())
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
    pub start_time: Time,
    pub end_time: Time,
}

fn filtered_data_route(
    data: Arc<GTFSData>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let cors = warp::cors().allow_any_origin();
    warp::path!("data" / String)
        .and(warp::query::<RadarOptions>())
        .and(with_data(data))
        .and(naive_state::with_session())
        .and_then(filtered_data_handler)
        .with(cors)
}

#[tokio::main]
async fn main() {
    let port = std::env::var("PORT")
        .unwrap_or_else(|_| "8080".to_owned())
        .parse()
        .unwrap();
    let static_dir = std::env::var("STATIC_DIR").unwrap_or_else(|_| "seed-frontend".to_owned());
    let gtfs_dir = std::env::var("GTFS_DIR").unwrap_or_else(|_| "gtfs".to_owned());
    let line_colors_path =
        std::env::var("LINE_COLORS").unwrap_or_else(|_| "./Linienfarben.csv".to_owned());
    let gtfs_dir = Path::new(&gtfs_dir);

    let colors = db::load_colors(Path::new(&line_colors_path)).expect(&line_colors_path);
    let data =
        Arc::new(db::load_data(&gtfs_dir, db::DayFilter::All, colors).expect("gtfs data to load"));
    let station_name_index = Arc::new(db::build_station_word_index(&*data));

    eprintln!("Starting web server on port {}", port);
    let log = warp::log("api");
    warp::serve(
        warp::fs::dir(static_dir.clone())
            .or(filtered_data_route(data.clone()))
            .or(endpoints::station_name_search_route(
                data.clone(),
                station_name_index,
            ))
            .or(warp::fs::file(format!("{}/index.html", &static_dir))) // for spa routing
            .with(log),
    )
    .run(([127, 0, 0, 1], port))
    .await;
}
