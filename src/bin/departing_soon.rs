use chrono::prelude::*;
use std::path::Path;

use radar_search::{search_data::*, time::*};
use transit_radar::gtfs::db;

fn main() {
    let gtfs_dir = std::env::var("GTFS_DIR").unwrap_or("gtfs".to_owned());
    let gtfs_dir = Path::new(&gtfs_dir);

    let data = db::load_data(&gtfs_dir, db::DayFilter::All, std::collections::HashMap::new()).unwrap();

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
    let station = db::get_station_by_name(&data, &"U Voltastr. (Berlin)").unwrap();

    let services = data.services_of_day(day);
    eprintln!("{} services", services.len());

    let trips = data.trips_from(station, &services, period);
    eprintln!("{:?}", trips);
    for child in station.children() {
        let stop = data.get_stop(child).unwrap();
        let trips = data.trips_from(&stop, &services, period);
        eprintln!("child {:?}", stop);
        for (trip, stops) in trips {
            eprintln!(
                "trip {} ({} {}), {:?}",
                trip.route.route_short_name, trip.trip_id, trip.service_id, stops
            );
        }
    }
}
