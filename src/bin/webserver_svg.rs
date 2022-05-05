use std::{borrow::Cow, collections::HashSet, fmt, io, num::NonZeroU64, path::Path, sync::Arc};

use chrono::{Duration, NaiveDateTime, TimeZone};
use radar_search::search_data::{Stop, StopId};
use rocket::{
    form::FromFormField,
    http::{ContentType, Status},
    request::FromParam,
    response::content,
    State,
};
use transit_radar::{
    draw::radar::{search, SearchParams, TransitMode, UrlSearchParams, STATION_ID_MIN},
    gtfs::db,
    write_xml, GTFSData, Suggester,
};

mod station_name_search;

#[macro_use]
extern crate rocket;

struct TransitModes(std::collections::HashSet<TransitMode>);

impl Default for TransitModes {
    fn default() -> Self {
        Self(
            [TransitMode::SBahn, TransitMode::UBahn]
                .iter()
                .copied()
                .collect(),
        )
    }
}

impl<'v> FromFormField<'v> for TransitModes {
    fn from_value(field: rocket::form::ValueField<'v>) -> rocket::form::Result<'v, Self> {
        if field.value.is_empty() {
            return Ok(Default::default());
        }
        let modes: HashSet<_> = field
            .value
            .split(',')
            .map(|mode| match mode {
                "ubahn" => Ok(TransitMode::UBahn),
                "sbahn" => Ok(TransitMode::SBahn),
                "bus" => Ok(TransitMode::Bus),
                "tram" => Ok(TransitMode::Tram),
                "regional" => Ok(TransitMode::Regional),
                "boat" => Ok(TransitMode::Boat),
                other => Err(rocket::form::Errors::from(
                    rocket::form::prelude::ErrorKind::InvalidChoice {
                        choices: vec![
                            "ubahn".into(),
                            "sbahn".into(),
                            "tram".into(),
                            "tram".into(),
                            "regional".into(),
                            "boat".into(),
                        ]
                        .into(),
                    },
                )
                .with_name(field.name)
                .with_value(other)),
            })
            .collect::<Result<_, _>>()?;
        Ok(TransitModes(modes))
    }

    fn default() -> Option<Self> {
        Some(Default::default())
    }
}

#[get("/depart-from/<id>/<time>?<minutes>&<refresh>&<mode>")]
fn index(
    id: u64,
    time: TimeFilter,
    minutes: Option<i64>,
    refresh: Option<bool>,
    mode: TransitModes,
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
        TimeFilter::Now => None,
        TimeFilter::Local(dt) => Some(chrono_tz::Europe::Berlin.from_local_datetime(&dt).unwrap()),
    };
    let max_duration = Duration::minutes(minutes.unwrap_or(30));
    let search_params = SearchParams {
        origin,
        departure_time,
        max_duration,
        modes: Cow::Borrowed(&mode.0),
    };
    let url_search_params = UrlSearchParams {
        station_id,
        departure_time,
        max_duration,
        modes: Cow::Borrowed(&mode.0),
    };
    let radar = search(data, search_params);
    let refresh = refresh.unwrap_or(false) && matches!(time, TimeFilter::Now);
    let mut svg = Vec::new();
    radar
        .write_svg_to(&mut io::Cursor::new(&mut svg), url_search_params, refresh)
        .unwrap();
    (ContentType::SVG, String::from_utf8(svg).unwrap())
}

#[get("/?<q>")]
fn station_search(
    q: Option<&str>,
    data: &State<Arc<GTFSData>>,
    suggester: &State<Suggester<(StopId, usize)>>,
) -> (Status, content::Html<String>) {
    let (status, main) = station_search_xml(q, data, suggester);
    let input_args: Cow<_> = if let Some(q) = q {
        if !q.is_empty() {
            format!(r#"value="{}""#, q).into()
        } else {
            "".into()
        }
    } else {
        "".into()
    };
    let page = format!(
        include_str!("station_search.html"),
        style = include_str!("style.css"),
        script = include_str!("script.js"),
        main = main,
        input_args = input_args
    );
    (status, content::Html(page))
}

#[get("/auto?<q>")]
fn station_search_xml(
    q: Option<&str>,
    data: &State<Arc<GTFSData>>,
    suggester: &State<Suggester<(StopId, usize)>>,
) -> (Status, String) {
    if let Some(q) = q {
        if let Ok(top_matches) = station_name_search::station_search_handler(q, &*data, &*suggester)
        {
            let mut string = String::new();
            write_results(&mut string, top_matches).unwrap();
            (Status::Ok, string)
        } else {
            (
                Status::BadRequest,
                "<main>Unable to parse query</main>".to_owned(),
            )
        }
    } else {
        (Status::Ok, "<main></main>".into())
    }
}

fn write_results<'s>(
    w: &mut dyn fmt::Write,
    matches: impl IntoIterator<Item = &'s Stop>,
) -> fmt::Result {
    write_xml!(w, <main>)?;
    for stop in matches {
        write_xml!(w,
            <a href={&format!("/depart-from/{id}/now", id = stop.stop_id)}>
                {stop.full_stop_name}
            </a>
        )?;
    }
    write_xml!(w, </main>)?;
    Ok(())
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

    let suggester = db::build_station_word_index(&data);

    rocket::build()
        .manage(data)
        .manage(suggester)
        .mount("/", routes![index, station_search, station_search_xml])
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Copy, Clone)]
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
