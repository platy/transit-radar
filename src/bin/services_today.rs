use chrono::prelude::*;
use std::path::Path;

use radar_search::{search_data::*, time::*};
use transit_radar::gtfs::db;

fn main() {
    let gtfs_dir = std::env::var("GTFS_DIR").unwrap_or("gtfs".to_owned());
    let gtfs_dir = Path::new(&gtfs_dir);

    let data = db::load_data(
        &gtfs_dir,
        db::DayFilter::All,
        std::collections::HashMap::new(),
    )
    .unwrap();

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
    // let station = db::get_station_by_name(&data, &station_name).unwrap();

    let services = data.services_of_day(day);
    eprintln!("{} services", services.len());

    let mut trips: Vec<_> = data
        .trips()
        .filter(|trip| {
            if trip.route.route_short_name == "U8" && services.contains(&trip.service_id) {
                true
            } else {
                false
            }
        })
        .collect();
    trips.sort_unstable_by_key(|trip| trip.stop_times[0].departure_time);
    trips.sort_by_key(|trip| trip.service_id);
    for trip in trips.iter() {
        eprintln!(
            "Service {}, Trip {}. {} - {}",
            trip.service_id,
            trip.trip_id,
            trip.stop_times[0].departure_time,
            trip.stop_times[trip.stop_times.len() - 1].arrival_time
        );
    }
    eprintln!("trips : {:?}", trips.len());
}
