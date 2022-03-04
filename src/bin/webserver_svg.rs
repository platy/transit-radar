use std::{io, path::Path, sync::Arc};

use rocket::{http::ContentType, State};
use transit_radar::{
    draw::radar::{search, Flags},
    gtfs::{db, StopId},
    GTFSData,
};

#[macro_use]
extern crate rocket;

#[get("/<id>")]
fn index(id: StopId, data: &State<Arc<GTFSData>>) -> (ContentType, String) {
    let origin = data.get_stop(id);
    let radar = search(
        data,
        origin.unwrap(),
        &Flags {
            show_ubahn: true,
            show_bus: false,
            show_regional: false,
            show_sbahn: true,
            show_tram: false,
        },
    );
    let mut svg = Vec::new();
    radar.write_svg_to(&mut io::Cursor::new(&mut svg)).unwrap();
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
