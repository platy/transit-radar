use std::{io, num::NonZeroU64, path::Path, sync::Arc};

use chrono::{Duration, NaiveDateTime, TimeZone, Utc};
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

#[get("/depart-from/<id>/<time>?<minutes>&<refresh>")]
fn index(
    id: u64,
    time: TimeFilter,
    minutes: Option<i64>,
    refresh: Option<bool>,
    data: &State<Arc<GTFSData>>,
) -> (ContentType, String) {
    let station_id = NonZeroU64::new(if id < STATION_ID_MIN {
        id + STATION_ID_MIN
    } else {
        id
    })
    .unwrap();
    let origin = data.get_stop(station_id).unwrap();
    assert!(origin.is_station(), "Origin must be a station");
    let departure_time = match time {
        TimeFilter::Now => Utc::now().with_timezone(&chrono_tz::Europe::Berlin),
        TimeFilter::Local(dt) => chrono_tz::Europe::Berlin.from_local_datetime(&dt).unwrap(),
    };
    let max_duration = Duration::minutes(minutes.unwrap_or(30));
    let radar = search(
        data,
        origin,
        departure_time,
        max_duration,
        &Flags {
            show_ubahn: true,
            show_bus: false,
            show_regional: false,
            show_sbahn: true,
            show_tram: false,
        },
    );
    let link_renderer = Box::new(
        |link_station_id: Option<StopId>, link_time: Option<NaiveDateTime>| {
            let link_station_id = link_station_id.unwrap_or(station_id);
            let link_time = link_time.map(TimeFilter::Local);
            format!(
                "/depart-from/{}/{}{}",
                link_station_id.get() - STATION_ID_MIN,
                link_time.as_ref().unwrap_or(&time),
                if let Some(minutes) = minutes {
                    std::borrow::Cow::Owned(format!("?minutes={}", minutes))
                } else {
                    "".into()
                }
            )
        },
    );
    let refresh = refresh.unwrap_or(true) && matches!(time, TimeFilter::Now);
    let mut svg = Vec::new();
    radar
        .write_svg_to(&mut io::Cursor::new(&mut svg), &link_renderer, refresh)
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
