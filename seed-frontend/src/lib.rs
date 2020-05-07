use std::collections::HashMap;
use seed::{*, prelude::*};
use web_sys::HtmlCanvasElement;
use radar_search::time::*;
use radar_search::search_data::*;
use radar_search::search_data_sync::*;
use radar_search::journey_graph;
use geo;
use std::f64::consts::PI;
use geo::algorithm::bearing::Bearing;
use js_sys;

#[derive(Default)]
struct Model {
    pub data: Option<GTFSData>,
    session_id: Option<u64>, // todo should be together with data and update count
    canvas: ElRef<HtmlCanvasElement>,
    canvas_scaled: Option<f64>,
    radar: Option<Radar>,

    controls: controls::Model,
}

struct Radar {
    geometry: RadarGeometry,
    trips: Vec<RadarTrip>,
    day: Day,
    expires_timestamp: f64,
}

struct RadarTrip {
    route_name: String,
    route_type: RouteType,
    route_color: String,
    connection: TripSegment,
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
        if duration.to_secs() < 0 {
            self.cartesian_origin
        } else {
            let bearing = self.geographic_origin.bearing(point);
            let h = duration.to_secs() as f64 / self.max_duration.to_secs() as f64;
            let x = h * (bearing * PI / 180.).cos();
            let y = h * (bearing * PI / 180.).sin();
            ((x+1.) * self.cartesian_origin.0, (-y+1.) * self.cartesian_origin.1)
        }
    }

    fn line_coords(&self, start_point: geo::Point<f64>, start_time: Time, end_point: geo::Point<f64>, end_time: Time) -> (f64, f64, f64, f64) {
        let (x1, y1) = if start_point == self.geographic_origin {
            // no bearing to the start point, so use the bearing of the end point
            self.coords(end_point, start_time)
        } else {
            self.coords(start_point, start_time)
        };
        let (x2, y2) = self.coords(end_point, end_time);
        (x1, y1, x2, y2)
    }

    fn initial_control_point(&self, (x1, y1): (f64, f64), (x2, y2): (f64, f64)) -> (f64, f64) {
        let (xo, yo) = self.cartesian_origin;
        let angle_to_origin = (yo - y1).atan2(xo - x1);
        let angle_to_next = (y2 - y1).atan2(x2 - x1);
        // things to adjust and improve
        let mut angle_between = angle_to_origin - angle_to_next;
        if angle_between < 0. { angle_between = 2. * PI + angle_between; }
        if angle_between < PI / 2. {
            let cpangle = angle_to_origin - PI / 2.;
            let cpmag = 0.5 * ((x1 - x2) * (x1 - x2) + (y1 - y2) * (y1 - y2)).sqrt();
            (x1 + cpmag * cpangle.cos(), y1 + cpmag * cpangle.sin())
        } else if angle_between > 3. * PI / 2. {
            let cpangle = angle_to_origin + PI / 2.;
            let cpmag = 0.5 * ((x1 - x2) * (x1 - x2) + (y1 - y2) * (y1 - y2)).sqrt();
            (x1 + cpmag * cpangle.cos(), y1 + cpmag * cpangle.sin())
        } else {
            (x1, y1)
        }
    }

    fn control_points(&self, (x1, y1): (f64, f64), (x2, y2): (f64, f64), (x3, y3): (f64, f64)) -> ((f64, f64), (f64, f64)) {
        let cpfrac = 0.3;
        let angle_to_prev = (y2 - y1).atan2(x2 - x1);
        let angle_to_next = (y2 - y3).atan2(x2 - x3);
        // things to adjust and improve
        let angle_to_tangent = (PI + angle_to_next + angle_to_prev) / 2.;
        let cp2mag = -cpfrac * ((x2 - x1) * (x2 - x1) + (y2 - y1) * (y2 - y1)).sqrt();
        let cp3mag = cpfrac * ((x2 - x3) * (x2 - x3) + (y2 - y3) * (y2 - y3)).sqrt();
        // ^^ improve these
        let mut dx = angle_to_tangent.cos();
        let mut dy = angle_to_tangent.sin();
        if angle_to_prev < angle_to_next {
          dy = -dy;
          dx = -dx;
        }
        ((x2 + (dx * cp2mag), y2 + (dy * cp2mag)), (x2 + (dx * cp3mag), y2 + (dy * cp3mag)))
    }
}


enum Msg {
    DataFetched(Result<GTFSDataSync, LoadError>),
    FetchData,
    Draw,
    ControlsMsg(controls::Msg),
}

#[derive(Debug)]
enum LoadError {
    FetchError(fetch::FetchError),
    RMPError(rmp_serde::decode::Error),
}

impl From<fetch::FetchError> for LoadError {
    fn from(error: fetch::FetchError) -> LoadError {
        Self::FetchError(error)
    }
}

impl From<rmp_serde::decode::Error> for LoadError {
    fn from(error: rmp_serde::decode::Error) -> LoadError {
        Self::RMPError(error)
    }
}

async fn fetch_data(id: Option<u64>) -> Result<GTFSDataSync, LoadError> {
    let session_part = id.map(|id| format!("&id={}&count=0", id)).unwrap_or_default();
    // todo use serde query params
    let url = format!("/data/U%20Voltastr.%20(Berlin)?ubahn=true&sbahn=true&bus=false&tram=false&regio=false{}", session_part);
    let response = fetch(url).await?;
    let body = response.bytes().await?;
    Ok(rmp_serde::from_read_ref(&body)?)
}

fn day_time(date_time: &js_sys::Date) -> (Day, Time) {
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
    let (day, start_time) = day_time(&js_sys::Date::new_0());
    let max_duration = Duration::minutes(30);
    let mut plotter = journey_graph::JourneyGraphPlotter::new(day, Period::between(start_time, start_time + max_duration), &data);
    let origin = data.get_stop(&900000007103).unwrap();
    plotter.add_origin_station(origin);
    plotter.add_route_type(RouteType::SuburbanRailway);
    plotter.add_route_type(RouteType::UrbanRailway);
    plotter.add_route_type(RouteType::TramService);
    let mut expires_time = start_time + max_duration;
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
                route_color,
            } => {
                expires_time = expires_time.min(departure_time);
                let trip = trips.get_mut(&trip_id).expect("trip to have been connected to");
                trip.segments.push(TripSegment {
                    from: from_stop.stop_id,
                    to: to_stop.stop_id,
                    departure_time,
                    arrival_time,
                });
            }
            journey_graph::Item::ConnectionToTrip {
                departure_time,
                arrival_time,
                from_stop,
                to_stop,
                trip_id,
                route_name,
                route_type,
                route_color,
            } => {
                trips.insert(trip_id, RadarTrip {
                    route_name: route_name.to_string(),
                    route_type,
                    route_color: route_color.to_owned(),
                    connection: TripSegment {
                        from: from_stop.stop_id,
                        to: to_stop.stop_id,
                        departure_time,
                        arrival_time,
                    },
                    segments: vec![],
                });
            }
        }
    }
    let expires_timestamp = js_sys::Date::new_0();
    expires_timestamp.set_hours(expires_time.hour() as u32);
    expires_timestamp.set_minutes(expires_time.minute() as u32);
    expires_timestamp.set_seconds(expires_time.second() as u32 + 1); // expire once this second is over
    expires_timestamp.set_milliseconds(0);
    let geometry = RadarGeometry {
        cartesian_origin: (500., 500.),
        geographic_origin: origin.location,
        start_time,
        max_duration,
    };
    let radar = Radar {
        day,
        expires_timestamp: expires_timestamp.value_of(),
        geometry,
        trips: trips.into_iter().map(|(_k,v)| v).collect(),
    };
    radar
}

fn update(msg: Msg, model: &mut Model, orders: &mut impl Orders<Msg>) {
    match msg {
        Msg::FetchData => {
            orders.perform_cmd(fetch_data(model.session_id).map(Msg::DataFetched));
            orders.skip();
        }

        Msg::DataFetched(Ok(data)) => {
            model.session_id = Some(data.session_id());
            let data = data.merge_data(&mut model.data);
            model.radar = Some(search(data));
            orders.after_next_render(|_| Msg::Draw);
        },

        Msg::DataFetched(Err(fail_reason)) => {
            error!(format!(
                "Fetch error - Fetching repository info failed - {:#?}",
                fail_reason
            ));
            orders.skip();
        },

        Msg::Draw => {
            if let &mut Some(ref mut radar) = &mut model.radar {
                let date = js_sys::Date::new_0();
                let (_day, time) = day_time(&date);
                if date.value_of() <= radar.expires_timestamp {
                    radar.geometry.start_time = time;
                } else {
                    model.radar = Some(search(model.data.as_ref().unwrap()));
                    orders.after_next_render(|_| Msg::FetchData);
                }
            } else {
                model.radar = Some(search(model.data.as_ref().unwrap()));
            }
            draw(model).unwrap();
            // The next time a render is triggered we will draw again
            orders.after_next_render(|_| Msg::Draw);
            if !model.controls.animate {
                orders.skip();
            }
        }

        Msg::ControlsMsg(msg) => controls::update(msg, &mut model.controls, &mut orders.proxy(Msg::ControlsMsg)),
    }
}

fn draw(model: &mut Model) -> Result<(), JsValue> { // todo , error type to encapsulate all the kinds of error in draw
    let canvas = model.canvas.get().expect("get canvas element");
    let ctx = seed::canvas_context_2d(&canvas);
    ctx.set_global_composite_operation("source-over").unwrap()  ;
    ctx.clear_rect(0., 0., 1200., 1000.);

    if model.canvas_scaled.is_none() {
        let scale = 2.; // epects scaling of 2, should be fine if the scaling is 1 also
        ctx.scale(scale, scale)?;
        model.canvas_scaled = Some(scale);
    }

    if let Some(Radar { day: _, expires_timestamp: _, geometry, trips, }) = &model.radar {
        let (origin_x, origin_y) = geometry.cartesian_origin;
        ctx.set_line_dash(&js_sys::Array::of2(&10f64.into(), &10f64.into()).into())?;
        ctx.set_stroke_style(&"lightgray".into());
        ctx.set_line_width(1.);
        ctx.begin_path();
        ctx.arc(origin_x, origin_y, 500. / 3., 0., 2. * std::f64::consts::PI)?;
        ctx.stroke();
        ctx.begin_path();
        ctx.arc(origin_x, origin_y, 500. * 2. / 3., 0., 2. * std::f64::consts::PI)?;
        ctx.stroke();
        ctx.begin_path();
        ctx.arc(origin_x, origin_y, 500., 0., 2. * std::f64::consts::PI)?;
        ctx.stroke();
        ctx.set_line_dash(&js_sys::Array::new().into())?;
        let data = model.data.as_ref().unwrap();

        for RadarTrip { connection, route_name: _, route_type, route_color, segments } in trips {
            {
                let TripSegment { from, to, departure_time, arrival_time } = connection;
                ctx.begin_path();
                ctx.set_line_width(1.);
                ctx.set_line_dash(&js_sys::Array::of2(&2f64.into(), &4f64.into()).into())?;
                ctx.set_stroke_style(&JsValue::from_str(route_color));
                let start_point = data.get_stop(from).unwrap().location;
                let mut end_point = data.get_stop(to).unwrap().location;
                if end_point == geometry.geographic_origin {
                    // connection is from origin meaning no natural bearing for it, we use the bearing to the next stop
                    end_point = data.get_stop(&segments.first().expect("at least one segment in a trip").to).unwrap().location;
                } 
                let (from_x, from_y, to_x, to_y) = geometry.line_coords(start_point, *departure_time, end_point, *arrival_time);
                ctx.move_to(to_x, to_y);
                ctx.line_to(from_x, from_y);
                ctx.stroke();
            }

            ctx.begin_path();
            use RouteType::*;
            if [Rail, RailwayService, SuburbanRailway, UrbanRailway, WaterTransportService].contains(route_type) {
                ctx.set_line_width(2.);
            } else { // Bus, BusService, TramService,
                ctx.set_line_width(1.);
            }
            ctx.set_line_dash(&js_sys::Array::new())?;
            ctx.set_stroke_style(&JsValue::from_str(route_color));
            if segments.len() > 1 {
                let mut next_control_point = { // first segment
                    let segment = &segments[0];
                    let (from_x, from_y, to_x, to_y) = geometry.line_coords(data.get_stop(&segment.from).unwrap().location, segment.departure_time, data.get_stop(&segment.to).unwrap().location, segment.arrival_time);
                    ctx.move_to(from_x, from_y);
                    let (cp1x, cp1y) = geometry.initial_control_point((from_x, from_y), (to_x, to_y));
                    let (post_id, post_time) = if segments[1].from != segment.to {
                        (segments[1].from, segments[1].departure_time)
                    } else {
                        (segments[1].to, segments[1].arrival_time)
                    };
                    let post_xy = geometry.coords(data.get_stop(&post_id).unwrap().location, post_time);
                    let ((cp2x, cp2y), next_control_point) = geometry.control_points((from_x, from_y), (to_x, to_y), post_xy);
                    ctx.bezier_curve_to(cp1x, cp1y, cp2x, cp2y, to_x, to_y);
                    next_control_point
                };
                // all the connecting segments
                for window in segments.windows(3) {
                    if let &[pre, segment, post] = &window {
                        let (from_x, from_y, to_x, to_y) = geometry.line_coords(data.get_stop(&segment.from).unwrap().location, segment.departure_time, data.get_stop(&segment.to).unwrap().location, segment.arrival_time);
                        let (pre_x, pre_y) = geometry.coords(data.get_stop(&pre.to).unwrap().location, pre.arrival_time);
                        if pre_x != from_x && pre_y != from_y {
                            // there is a gap in the route and so we move
                            ctx.move_to(from_x, from_y);
                        }
                        let (post_id, post_time) = if post.from != segment.to {
                            (post.from, post.departure_time)
                        } else {
                            (post.to, post.arrival_time)
                        };
                        let post_xy = geometry.coords(data.get_stop(&post_id).unwrap().location, post_time);
                        let (cp1x, cp1y) = next_control_point;
                        let ((cp2x, cp2y), next_control_point_tmp) = geometry.control_points((from_x, from_y), (to_x, to_y), post_xy);
                        ctx.bezier_curve_to(cp1x, cp1y, cp2x, cp2y, to_x, to_y);
                        next_control_point = next_control_point_tmp;
                    } else {
                        panic!("unusual window");
                    }
                }
                {
                    // draw the last curve
                    let end = segments.last().unwrap();
                    let (to_x, to_y) = geometry.coords(data.get_stop(&end.from).unwrap().location, end.departure_time);
                    let (cp1x, cp1y) = next_control_point;
                    ctx.bezier_curve_to(cp1x, cp1y, to_x, to_y, to_x, to_y);
                }
            } else {
                let segment = &segments[0];
                let (from_x, from_y, to_x, to_y) = geometry.line_coords(data.get_stop(&segment.from).unwrap().location, segment.departure_time, data.get_stop(&segment.to).unwrap().location, segment.arrival_time);
                ctx.move_to(from_x, from_y);
                ctx.line_to(to_x, to_y);
            }
            ctx.stroke();
        }
    }
    Ok(())
}

mod controls {
    use seed::{*, prelude::*};

    #[derive(Default)]
    pub struct Model {
        pub show_stations: bool,
        pub animate: bool,
        pub show_sbahn: bool,
        pub show_ubahn: bool,
        pub show_bus: bool,
        pub show_tram: bool,
        pub show_regional: bool,
    }

    pub fn view(model: &Model) -> Vec<Node<Msg>> {
        nodes![
            checkbox("show-stations", "Show Stations", model.show_stations, &Msg::SetShowStations),
            checkbox("animate", "Animate", model.animate, &Msg::SetAnimate),
            checkbox("show-sbahn", "Show SBahn", model.show_sbahn, &Msg::SetShowSBahn),
            checkbox("show-ubahn", "Show UBahn", model.show_ubahn, &Msg::SetShowUBahn),
            checkbox("show-bus", "Show Bus", model.show_bus, &Msg::SetShowBus),
            checkbox("show-tram", "Show Tram", model.show_tram, &Msg::SetShowTram),
            checkbox("show-regional", "Show Regional", model.show_tram, &Msg::SetShowRegional),
        ]
    }

    pub enum Msg {
        SetShowStations(String),
        SetAnimate(String),
        SetShowSBahn(String),
        SetShowUBahn(String),
        SetShowBus(String),
        SetShowTram(String),
        SetShowRegional(String),
    }

    pub fn update(msg: Msg, model: &mut Model, _orders: &mut impl Orders<Msg>) {
        match msg {
            Msg::SetShowStations(_value) => model.show_stations = ! model.show_stations,
            Msg::SetAnimate(_value) => model.animate = ! model.animate,
            Msg::SetShowSBahn(_value) => model.show_sbahn = ! model.show_sbahn,
            Msg::SetShowUBahn(_value) => model.show_ubahn = ! model.show_ubahn,
            Msg::SetShowBus(_value) => model.show_bus = ! model.show_bus,
            Msg::SetShowTram(_value) => model.show_tram = ! model.show_tram,
            Msg::SetShowRegional(_value) => model.show_regional = ! model.show_regional,
        }
    }

    fn checkbox<M>(name: &'static str, label: &'static str, value: bool, event: &'static M) -> Vec<Node<Msg>>
        where M: FnOnce(String) -> Msg + Copy {
        vec![
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
}

fn view(model: &Model) -> Node<Msg> {
    if let Some(data) = &model.data {
        div![
            h2!["U Voltastrasse"],
            controls::view(&model.controls).map_msg(Msg::ControlsMsg),
            canvas![
                el_ref(&model.canvas),
                attrs![
                    At::Width => px(2400),
                    At::Height => px(2000),
                ],
                style![
                    St::Border => "1px solid black",
                    St::Width => px(1200),
                    St::Height => px(1000),
                ],
            ],
            if let Some(radar) = &model.radar {
                format!("data processed for {}, {}. {} trips", radar.day, radar.geometry.start_time, radar.trips.len())
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
