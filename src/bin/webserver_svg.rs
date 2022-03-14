use std::{io, num::NonZeroU64, path::Path, sync::Arc};

use chrono::{NaiveDateTime, TimeZone, Utc};
use radar_search::search_data::StopId;
use rocket::{http::ContentType, request::FromParam, State};
use transit_radar::{
    draw::radar::{search, Flags},
    gtfs::db,
    GTFSData,
};

#[macro_use]
extern crate rocket;

const STATION_ID_MIN: u64 = 900_000_000_000;

#[get("/depart-from/<id>/<time>")]
fn index(mut id: u64, time: TimeFilter, data: &State<Arc<GTFSData>>) -> (ContentType, String) {
    if id < STATION_ID_MIN {
        id = id + STATION_ID_MIN;
    }
    let origin = data.get_stop(NonZeroU64::new(id).unwrap()).unwrap();
    assert!(origin.is_station(), "Origin must be a station");
    let departure_time = match time {
        TimeFilter::Now => Utc::now().with_timezone(&chrono_tz::Europe::Berlin),
        TimeFilter::Local(dt) => chrono_tz::Europe::Berlin.from_local_datetime(&dt).unwrap(),
    };
    let radar = search(
        data,
        origin,
        departure_time,
        &Flags {
            show_ubahn: true,
            show_bus: false,
            show_regional: false,
            show_sbahn: true,
            show_tram: false,
        },
    );
    let link_renderer = Box::new(|station_id: StopId| {
        format!(
            "/depart-from/{}/{}",
            station_id.get() - STATION_ID_MIN,
            &time
        )
    });
    let mut svg = Vec::new();
    radar
        .write_svg_to(&mut io::Cursor::new(&mut svg), &link_renderer)
        .unwrap();
    (ContentType::SVG, String::from_utf8(svg).unwrap())
}

#[launch]
fn rocket() -> _ {
    let gtfs_dir = std::env::var("GTFS_DIR").unwrap_or_else(|_| "gtfs".to_owned());
    let line_colors_path =
        std::env::var("LINE_COLORS").unwrap_or_else(|_| "./VBB_Colours.csv".to_owned());
    let gtfs_dir = Path::new(&gtfs_dir);

    let colors = db::load_colors(Path::new(&line_colors_path)).expect(&line_colors_path);
    let data =
        Arc::new(db::load_data(gtfs_dir, db::DayFilter::All, colors).expect("gtfs data to load"));

    rocket::build().manage(data).mount("/", routes![index])
}

enum TimeFilter {
    Now,
    Local(NaiveDateTime),
}

impl<'a> FromParam<'a> for TimeFilter {
    type Error = chrono::format::ParseError;

    fn from_param(param: &'a str) -> Result<Self, Self::Error> {
        if param == "now" {
            Ok(Self::Now)
        } else {
            param.parse().map(Self::Local)
        }
    }
}

impl std::fmt::Display for TimeFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TimeFilter::Now => f.write_str("now"),
            TimeFilter::Local(dt) => std::fmt::Debug::fmt(dt, f),
        }
    }
}
