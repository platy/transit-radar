use chrono::prelude::*;
use std::path::Path;
use std::sync::Arc;
use serde::Serialize;
use warp::Filter;
use urlencoding::decode;

use radar_search::{search_data::*, time::*, journey_graph};
use transit_radar::gtfs::db;

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
) -> Result<impl warp::Reply, warp::Rejection> {
    let (day, now) = day_time(chrono::Utc::now());
    let period = Period::between(now, now + Duration::minutes(30));

    match decode(&name) {
        Ok(name) => {
            let data = filter_data(&data, name, options, day, period).map_err(warp::reject::custom)?;
            let mut buf = Vec::<u8>::new();
            data.serialize(
                &mut rmp_serde::Serializer::new(&mut buf)
                    .with_struct_tuple()
                    .with_integer_variants(),
            ).map_err(|err| {
                eprintln!("failed to serialize data {:?}", err);
                warp::reject::reject()
            })?;
            Ok(buf)
        },
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
    warp::serve(
        warp::fs::dir(static_dir)
            .or(filtered_data_route(data.clone()))
    )
    .run(([127, 0, 0, 1], port))
    .await;
}
