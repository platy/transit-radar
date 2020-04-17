use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use warp::Filter;
use urlencoding::decode;
use chrono::prelude::*;

use transit_radar::journey_graph;
use transit_radar::GTFSData;
use transit_radar::gtfs::*;
use transit_radar::gtfs::db;

use geo::algorithm::bearing::Bearing;

fn lookup<'r>(data: &'r GTFSData, station_name: String, options: RadarOptions, day: Day, period: Period) -> Result<(), db::SearchError> {
    let station = db::get_station_by_name(data, &station_name)?;
    produce_tree_json(&data, station.stop_id, day, period, &options);
    Ok(())
}

fn produce_tree_json<'r>(data: &'r GTFSData, station: StopId, day: Day, period: Period, options: &RadarOptions) {
    let mut plotter = journey_graph::JourneyGraphPlotter::new(day, period, data);
    let origin = data.get_stop(&station).unwrap();
    plotter.add_origin_station(origin);
    if options.ubahn { plotter.add_route_type(RouteType::UrbanRailway); }
    if options.sbahn { plotter.add_route_type(RouteType::SuburbanRailway); }
    if options.bus { plotter.add_route_type(RouteType::BusService); }
    if options.tram { plotter.add_route_type(RouteType::TramService); }
    if options.regio { plotter.add_route_type(RouteType::RailwayService); }
    if options.bus { plotter.add_route_type(RouteType::Bus); }

    for item in plotter {
        match item {
            journey_graph::Item::Station {
                stop,
                earliest_arrival,
            } => {
            },
            journey_graph::Item::JourneySegment {
                departure_time, 
                arrival_time, 
                from_stop,
                to_stop,
            } => {
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
            },
            journey_graph::Item::ConnectionToTrip {
                departure_time, 
                arrival_time, 
                from_stop,
                to_stop,
                route_name,
                route_type,
            } => {
            },
        }
    }
}

fn search(name: String, options: RadarOptions, data: &GTFSData) {
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

    lookup(data, name, options, day, period).unwrap();
}

#[derive(Debug, serde::Deserialize)]
pub struct RadarOptions {
    pub ubahn: bool,
    pub sbahn: bool,
    pub bus: bool,
    pub regio: bool,
    pub tram: bool,
}

fn main() {
    let gtfs_dir = std::env::var("GTFS_DIR").unwrap_or("gtfs".to_owned());
    let gtfs_dir = Path::new(&gtfs_dir);

    let data = db::load_data(
        &gtfs_dir,
        db::DayFilter::All, 
    ).unwrap();

    search("U Voltastr. (Berlin)".to_owned(), RadarOptions { ubahn: true, sbahn: false, bus: false, regio: false, tram: false}, &data);
}
