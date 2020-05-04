use std::collections::HashMap;
use seed::{*, prelude::*};
use web_sys::HtmlCanvasElement;
use radar_search::time::*;
use radar_search::search_data::*;
use radar_search::journey_graph;
use geo;
use std::f64::consts::PI;
use geo::algorithm::bearing::Bearing;
use js_sys;

#[derive(Default)]
struct Model {
    pub data: Option<GTFSData>,
    canvas: ElRef<HtmlCanvasElement>,
    radar: Option<Radar>,

    show_stations: bool,
    animate: bool,
    show_sbahn: bool,
    show_ubahn: bool,
    show_bus: bool,
    show_tram: bool,
    show_regional: bool,
}

struct Radar {
    geometry: RadarGeometry,
    trips: Vec<RadarTrip>,
    day: Day,
    start_time: Time,
}

struct RadarTrip {
    route_name: String,
    route_type: RouteType,
    segments: Vec<TripSegment>,
}

struct TripSegment {
    from: StopId,
    to: StopId,
    departure_time: Time,
    arrival_time: Time,
}

struct RadarGeometry {
    start_time: Time,
    cartesian_origin: (f64, f64),
    geographic_origin: geo::Point<f64>,
    max_duration: Duration,
}

impl RadarGeometry {
    fn coords(&self, point: geo::Point<f64>, time: Time) -> (f64, f64) {
        let duration = time - self.start_time;
        if duration.to_secs() == 0 {
            self.cartesian_origin
        } else {
            let bearing = self.geographic_origin.bearing(point);
            let h = duration.to_secs() as f64 / self.max_duration.to_secs() as f64;
            let x = h * (bearing * PI / 180.).cos();
            let y = h * (bearing * PI / 180.).sin();
            ((x+1.) * self.cartesian_origin.0, (y+1.) * self.cartesian_origin.1)
        }
    }
}


enum Msg {
    DataFetched(Result<GTFSData, Box<dyn std::error::Error>>),
    FetchData,
    Rendered,
    SetShowStations(String),
    SetAnimate(String),
    SetShowSBahn(String),
    SetShowUBahn(String),
    SetShowBus(String),
    SetShowTram(String),
    SetShowRegional(String),
}

async fn fetch_data() -> Result<GTFSData, Box<dyn std::error::Error>> {
    let url = "/search-data.messagepack";
    let response = fetch(url).await?;
    let body = response.bytes().await?;
    Ok(rmp_serde::from_read_ref(&body)?)
}

fn day_time(date_time: js_sys::Date) -> (Day, Time) {
    let now = Time::from_hms(date_time.get_hours(), date_time.get_minutes(), date_time.get_seconds());
    let day = match date_time.get_day() {
        1 => Day::Monday,
        2 => Day::Tuesday,
        3 => Day::Wednesday,
        4 => Day::Thursday,
        5 => Day::Friday,
        6 => Day::Saturday,
        0 => Day::Sunday,
        x => panic!("Unknown day : {}", x),
    };
    (day, now)
}

fn search(data: &GTFSData) -> Radar {
    // TODO don't use client time, instead start with the server time and increment using client clock, also this is local time
    let (day, start_time) = day_time(js_sys::Date::new_0());
    let max_duration = Duration::minutes(30);
    let mut plotter = journey_graph::JourneyGraphPlotter::new(day, Period::between(start_time, start_time + max_duration), &data);
    let origin = data.get_stop(&900000007103).unwrap();
    plotter.add_origin_station(origin);
    plotter.add_route_type(RouteType::UrbanRailway);
    let mut trips: HashMap<TripId, RadarTrip> = HashMap::new();
    for item in plotter {
        match item {
            journey_graph::Item::Station {
                stop,
                earliest_arrival,
            } => {
                // FEStop {
                //     bearing: origin.location.bearing(stop.location),
                //     name: stop.stop_name.replace(" (Berlin)", ""),
                //     seconds: (earliest_arrival - period.start()).to_secs(),
                // });
            }
            journey_graph::Item::JourneySegment {
                departure_time,
                arrival_time,
                from_stop,
                to_stop,
            } => {
                // let to = *stop_id_to_idx.get(&to_stop.station_id()).unwrap();
                // let from_stop_or_station_id = from_stop.station_id();
                // let from = *stop_id_to_idx.get(&from_stop_or_station_id).unwrap_or(&to);
                // fe_conns.push(FEConnection {
                //     from,
                //     to,
                //     route_name: None,
                //     kind: None,
                //     from_seconds: (departure_time - period.start()).to_secs(),
                //     to_seconds: (arrival_time - period.start()).to_secs(),
                // })
            }
            journey_graph::Item::SegmentOfTrip {
                departure_time,
                arrival_time,
                from_stop,
                to_stop,
                trip_id,
                route_name,
                route_type,
            } => {
                let trip = trips.entry(trip_id).or_insert(RadarTrip {
                    route_name: route_name.to_string(),
                    route_type,
                    segments: vec![],
                });
                trip.segments.push(TripSegment {
                    from: from_stop.station_id(),
                    to: to_stop.station_id(),
                    departure_time,
                    arrival_time,
                });
            }
            journey_graph::Item::ConnectionToTrip {
                departure_time,
                arrival_time,
                from_stop,
                to_stop,
                route_name,
                route_type,
            } => {
                // let to = *stop_id_to_idx.get(&to_stop.station_id()).unwrap();
                // let from_stop_or_station_id = from_stop.station_id();
                // let from = *stop_id_to_idx.get(&from_stop_or_station_id).unwrap_or(&to);
                // fe_conns.push(FEConnection {
                //     from,
                //     to,
                //     route_name: Some(route_name),
                //     kind: Some(FEConnectionType::from(route_type)),
                //     from_seconds: (departure_time - period.start()).to_secs(),
                //     to_seconds: (arrival_time - period.start()).to_secs(),
                // })
            }
        }
    }
    let geometry = RadarGeometry {
        cartesian_origin: (500., 500.),
        geographic_origin: origin.location,
        start_time,
        max_duration,
    };
    let radar = Radar {
        day,
        start_time,
        geometry,
        trips: trips.into_iter().map(|(_k,v)| v).collect(),
    };
    radar
}

fn update(msg: Msg, model: &mut Model, orders: &mut impl Orders<Msg>) {
    match msg {
        Msg::FetchData => {
            orders.perform_cmd(fetch_data().map(Msg::DataFetched));
            orders.skip();
        }

        Msg::DataFetched(Ok(data)) => {
            model.data = Some(data);
            orders.after_next_render(|_| Msg::Rendered);
        },

        Msg::DataFetched(Err(fail_reason)) => {
            error!(format!(
                "Fetch error - Fetching repository info failed - {:#?}",
                fail_reason
            ));
            orders.skip();
        },

        Msg::Rendered => {
            model.radar = Some(search(model.data.as_ref().unwrap()));
            draw(&model.canvas, &model);
            // We want to call `.skip` to prevent infinite loop.
            // (Infinite loops are useful for animations.)
            orders.after_next_render(|_| Msg::Rendered);
            if !model.animate {
                orders.skip();
            }
        }

        Msg::SetShowStations(value) => model.show_stations = ! model.show_stations,
        Msg::SetAnimate(value) => model.animate = ! model.animate,
        Msg::SetShowSBahn(value) => model.show_sbahn = ! model.show_sbahn,
        Msg::SetShowUBahn(value) => model.show_ubahn = ! model.show_ubahn,
        Msg::SetShowBus(value) => model.show_bus = ! model.show_bus,
        Msg::SetShowTram(value) => model.show_tram = ! model.show_tram,
        Msg::SetShowRegional(value) => model.show_regional = ! model.show_regional,
    }
}

fn draw(canvas: &ElRef<HtmlCanvasElement>, model: &Model) {
    let canvas = canvas.get().expect("get canvas element");
    let ctx = seed::canvas_context_2d(&canvas);
    ctx.set_global_composite_operation("source-over").unwrap()  ;
    ctx.clear_rect(0., 0., 1200., 1000.);

    if let Some(Radar { day, start_time, geometry, trips }) = &model.radar {
        let data = model.data.as_ref().unwrap();
        for RadarTrip { route_name, route_type, segments } in trips {
            ctx.begin_path();
            for segment in segments {
                let (from_x, from_y) = geometry.coords(data.get_stop(&segment.from).unwrap().location, segment.departure_time);
                let (to_x, to_y) = geometry.coords(data.get_stop(&segment.to).unwrap().location, segment.arrival_time);
                ctx.move_to(from_x, from_y);
                ctx.line_to(to_x, to_y);
            }
            ctx.stroke();
        }
    }
}

fn checkbox<M>(name: &'static str, label: &'static str, value: bool, event: &'static M) -> [Node<Msg>; 2] 
    where M: FnOnce(String) -> Msg + Copy {
    [
        input![ attrs!{
            At::Type => "checkbox",
            At::Checked => value.as_at_value(),
            At::Name => name,
        }, input_ev(Ev::Input, *event)],
        label![ attrs!{
            At::For => name
        }, label],
    ]
}

fn view(model: &Model) -> Node<Msg> {
    if let Some(data) = &model.data {
        div![
            h2!["U Voltastrasse"],
            checkbox("show-stations", "Show Stations", model.show_stations, &Msg::SetShowStations),
            checkbox("animate", "Animate", model.animate, &Msg::SetAnimate),
            checkbox("show-sbahn", "Show SBahn", model.show_sbahn, &Msg::SetShowSBahn),
            checkbox("show-ubahn", "Show UBahn", model.show_ubahn, &Msg::SetShowUBahn),
            checkbox("show-bus", "Show Bus", model.show_bus, &Msg::SetShowBus),
            checkbox("show-tram", "Show Tram", model.show_tram, &Msg::SetShowTram),
            checkbox("show-regional", "Show Regional", model.show_tram, &Msg::SetShowRegional),
            
            canvas![
                el_ref(&model.canvas),
                attrs![
                    At::Width => px(1200),
                    At::Height => px(1000),
                ],
                style![
                    St::Border => "1px solid black",
                ],
            ],
            if let Some(radar) = &model.radar {
                format!("data processed for {}, {}. {} trips", radar.day, radar.start_time, radar.trips.len())
            } else {
                format!("data received, {} stops", data.stops().count())
            }
        ]
    } else {
        div!["Data not loaded"]
    }
}

fn after_mount(_: Url, orders: &mut impl Orders<Msg>) -> AfterMount<Model> {
    orders.send_msg(Msg::FetchData);
    AfterMount::default()
}

#[wasm_bindgen(start)]
pub fn render() {
    App::builder(update, view)
        .after_mount(after_mount)
        .build_and_start();
}
