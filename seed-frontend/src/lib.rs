use geo;
use geo::algorithm::bearing::Bearing;
use js_sys;
use radar_search::journey_graph;
use radar_search::search_data::*;
use radar_search::search_data_sync::*;
use radar_search::time::*;
use seed::{prelude::*, *};
use std::collections::HashMap;
use std::f64::consts::PI;

mod canvasser;
mod controls;
mod sync;

#[wasm_bindgen(start)]
pub fn render() {
    App::builder(update, view)
        .after_mount(after_mount)
        .build_and_start();
}

fn after_mount(_: Url, orders: &mut impl Orders<Msg>) -> AfterMount<Model> {
    orders.after_next_render(|_| Msg::Rendered);
    AfterMount::default()
}

/// fn after_mount(_: Url, orders: &mut impl Orders<Msg>) -> AfterMount<Model> {
///     orders.after_next_render(|_| Msg::Rendered);
/// // ...
///
/// fn update(msg: Msg, model: &mut Model, orders: &mut impl Orders<Msg>) {
///     match msg {
///         Msg::Rendered => {
///             let canvas = canvas.get().expect("get canvas element");
///             // ...
///             orders.after_next_render(|_| Msg::Rendered).skip();
///         }

struct Model {
    canvasser: canvasser::App<CanvasMsg, CanvasModel, Vec<Box<dyn Drawable>>>, // going to need to find a workaround to avoid the view return type in here
    controls: controls::Model,
}

impl Default for Model {
    fn default() -> Model {
        Model {
            canvasser: canvasser::App::builder(canvas_update, canvas_view)
                .canvas_added(|| CanvasMsg::CanvasAdded)
                .build(),
            controls: controls::Model::default(),
        }
    }
}

enum Msg {
    /// When a user changes a control
    ControlsMsg(controls::Msg),
    /// After each render is completed
    Rendered,
}

#[derive(serde::Serialize)]
struct Params {
    ubahn: bool,
    sbahn: bool,
    bus: bool,
    tram: bool,
    regio: bool,
}

fn update(msg: Msg, model: &mut Model, orders: &mut impl Orders<Msg>) {
    match msg {
        Msg::ControlsMsg(msg) => {
            controls::update(
                msg,
                &mut model.controls,
                &mut orders.proxy(Msg::ControlsMsg),
            );
            model
                .canvasser
                .update(CanvasMsg::ControlsChange(model.controls.clone()));
        }
        Msg::Rendered => {
            model.canvasser.rendered();
            orders.after_next_render(|_| Msg::Rendered).skip();
        }
    }
}

fn view(model: &Model) -> Node<Msg> {
    // if let Some(data) = model.sync.get() {
    div![
        h2!["U Voltastrasse"],
        controls::view(&model.controls).map_msg(Msg::ControlsMsg),
        div![canvas![
            &model.canvasser.el_ref(),
            attrs![
                At::Width => px(2400),
                At::Height => px(2000),
            ],
            style![
                St::Border => "1px solid black",
                St::Width => px(1200),
                St::Height => px(1000),
            ],
        ]],
        // if let Some(radar) = &model.radar {
        //     format!(
        //         "data processed for {}, {}. {} trips",
        //         radar.day,
        //         radar.geometry.start_time,
        //         radar.trips.len()
        //     )
        // } else {
        //     format!("data received, {} stops", data.stops().count())
        // }
    ]
    // } else {
    //     div!["Data not loaded"]
    // }
}

#[derive(Default)]
struct CanvasModel {
    sync: sync::Model<GTFSData>,
    radar: Option<Radar>,

    controls: controls::Model,
}

enum CanvasMsg {
    AnimationFrame,
    CanvasAdded,
    SyncMsg(sync::Msg<GTFSData, GTFSSyncIncrement>),
    Search,
    SearchExpires,
    ControlsChange(controls::Model),
}

fn canvas_update(
    msg: CanvasMsg,
    model: &mut CanvasModel,
    orders: &mut impl canvasser::Orders<CanvasMsg>,
) {
    match msg {
        CanvasMsg::CanvasAdded => {
            if model.sync.never_requested() {
                orders.send_msg(CanvasMsg::SyncMsg(sync::Msg::FetchData));
            }
        }
        CanvasMsg::SyncMsg(msg) => {
            let query = serde_urlencoded::to_string(Params {
                ubahn: model.controls.show_ubahn,
                sbahn: model.controls.show_sbahn,
                tram: model.controls.show_tram,
                regio: model.controls.show_regional,
                bus: model.controls.show_bus,
            })
            .unwrap();
            let url = format!("/data/U%20Voltastr.%20(Berlin)?{}", query);
            if sync::update(
                msg,
                &mut model.sync,
                url,
                &mut orders.proxy(CanvasMsg::SyncMsg),
            ) {
                orders.send_msg(CanvasMsg::Search);
            }
        }

        CanvasMsg::Search => {
            let result = search(model.sync.get().unwrap(), &model.controls);
            orders.schedule_msg(result.expires_timestamp, CanvasMsg::SearchExpires);
            model.radar = Some(result);
        }

        CanvasMsg::SearchExpires => {
            let date = js_sys::Date::new_0();
            let radar = model.radar.as_mut().unwrap();
            // check whether it is actually expired, it may have been updated before this message was scheduled
            if date.value_of() as u64 > radar.expires_timestamp {
                orders.send_msg(CanvasMsg::Search);
                orders.send_msg(CanvasMsg::SyncMsg(sync::Msg::FetchData));
            }
        }

        CanvasMsg::AnimationFrame => {
            if model.controls.animate {
                orders.next_frame_end(|_| CanvasMsg::AnimationFrame);
            }

            // by default, we skip the render, unless the clock has changed to actually cause a change
            orders.skip();
            if let Some(radar) = &mut model.radar {
                let date = js_sys::Date::new_0();
                let (_day, time) = day_time(&date);

                if time != radar.geometry.start_time {
                    radar.geometry.start_time = time;
                    orders.render();
                }
            }
        }

        CanvasMsg::ControlsChange(controls) => {
            if controls.animate && !model.controls.animate {
                orders.send_msg(CanvasMsg::AnimationFrame);
            }
            model.controls = controls;
            orders.send_msg(CanvasMsg::Search);
            orders.send_msg(CanvasMsg::SyncMsg(sync::Msg::FetchData));
        }
    }
}

struct Radar {
    geometry: RadarGeometry,
    trips: Vec<RadarTrip>,
    stations: Vec<Station>,
    day: Day,
    expires_timestamp: u64,
}

struct Station {
    location: geo::Point<f64>,
    earliest_arrival: Time,
    name: String,
}

struct RadarTrip {
    // route_name: String,
    route_type: RouteType,
    route_color: String,
    connection: TripSegment,
    segments: Vec<TripSegment>,
}

struct TripSegment {
    from: geo::Point<f64>,
    to: geo::Point<f64>,
    departure_time: Time,
    arrival_time: Time,
}

// needs to be cloneable for the view, could be avoided
#[derive(Clone)]
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
            (
                (x + 1.) * self.cartesian_origin.0,
                (-y + 1.) * self.cartesian_origin.1,
            )
        }
    }

    fn line_coords(
        &self,
        start_point: geo::Point<f64>,
        start_time: Time,
        end_point: geo::Point<f64>,
        end_time: Time,
    ) -> (f64, f64, f64, f64) {
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
        if angle_between < 0. {
            angle_between = 2. * PI + angle_between;
        }
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

    fn control_points(
        &self,
        (x1, y1): (f64, f64),
        (x2, y2): (f64, f64),
        (x3, y3): (f64, f64),
    ) -> ((f64, f64), (f64, f64)) {
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
        (
            (x2 + (dx * cp2mag), y2 + (dy * cp2mag)),
            (x2 + (dx * cp3mag), y2 + (dy * cp3mag)),
        )
    }
}

fn day_time(date_time: &js_sys::Date) -> (Day, Time) {
    let now = Time::from_hms(
        date_time.get_hours(),
        date_time.get_minutes(),
        date_time.get_seconds(),
    );
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

fn search(data: &GTFSData, controls: &controls::Model) -> Radar {
    // TODO don't use client time, instead start with the server time and increment using client clock, also this is local time
    let (day, start_time) = day_time(&js_sys::Date::new_0());
    let max_duration = Duration::minutes(30);
    let mut plotter = journey_graph::JourneyGraphPlotter::new(
        day,
        Period::between(start_time, start_time + max_duration),
        &data,
    );
    let origin = data.get_stop(&900000007103).unwrap();
    plotter.add_origin_station(origin);
    if controls.show_sbahn {
        plotter.add_route_type(RouteType::SuburbanRailway)
    }
    if controls.show_ubahn {
        plotter.add_route_type(RouteType::UrbanRailway)
    }
    if controls.show_bus {
        plotter.add_route_type(RouteType::BusService);
        plotter.add_route_type(RouteType::Bus)
    }
    if controls.show_tram {
        plotter.add_route_type(RouteType::TramService)
    }
    if controls.show_regional {
        plotter.add_route_type(RouteType::RailwayService)
    }
    let mut expires_time = start_time + max_duration;
    let mut trips: HashMap<TripId, RadarTrip> = HashMap::new();
    let mut stations = vec![];
    for item in plotter {
        match item {
            journey_graph::Item::Station {
                stop,
                earliest_arrival,
            } => {
                stations.push(Station {
                    location: stop.location,
                    name: stop.stop_name.replace(" (Berlin)", ""),
                    earliest_arrival,
                });
            }
            journey_graph::Item::JourneySegment {
                departure_time: _,
                arrival_time: _,
                from_stop: _,
                to_stop: _,
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
                route_name: _,
                route_type: _,
                route_color: _,
            } => {
                expires_time = expires_time.min(departure_time);
                let trip = trips
                    .get_mut(&trip_id)
                    .expect("trip to have been connected to");
                trip.segments.push(TripSegment {
                    from: from_stop.location,
                    to: to_stop.location,
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
                trips.insert(
                    trip_id,
                    RadarTrip {
                        // route_name: route_name.to_string(),
                        route_type,
                        route_color: route_color.to_owned(),
                        connection: TripSegment {
                            from: from_stop.location,
                            to: to_stop.location,
                            departure_time,
                            arrival_time,
                        },
                        segments: vec![],
                    },
                );
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
        expires_timestamp: expires_timestamp.value_of() as u64,
        geometry,
        trips: trips.into_iter().map(|(_k, v)| v).collect(),
        stations,
    };
    radar
}

use canvasser::{Drawable, Path, Circle, Text};

impl Drawable for RadarGeometry {
    fn draw(&self, ctx: &web_sys::CanvasRenderingContext2d) {
        let (origin_x, origin_y) = self.cartesian_origin;
        ctx.set_line_dash(&js_sys::Array::of2(&10f64.into(), &10f64.into()).into())
            .unwrap();
        ctx.set_stroke_style(&"lightgray".into());
        ctx.set_line_width(1.);
        ctx.begin_path();
        ctx.arc(origin_x, origin_y, 500. / 3., 0., 2. * std::f64::consts::PI)
            .unwrap();
        ctx.stroke();
        ctx.begin_path();
        ctx.arc(
            origin_x,
            origin_y,
            500. * 2. / 3.,
            0.,
            2. * std::f64::consts::PI,
        )
        .unwrap();
        ctx.stroke();
        ctx.begin_path();
        ctx.arc(origin_x, origin_y, 500., 0., 2. * std::f64::consts::PI)
            .unwrap();
        ctx.stroke();
        ctx.set_line_dash(&js_sys::Array::new().into()).unwrap();
    }
}

fn canvas_view(model: &CanvasModel) -> Vec<Box<dyn Drawable>> {
    if model.radar.is_none() {
        return vec![];
    }
    let Radar {
        day: _,
        expires_timestamp: _,
        geometry,
        trips,
        stations,
    } = model.radar.as_ref().unwrap();

    let mut paths: Vec<Box<dyn Drawable>> = vec![Box::new(geometry.clone())];
    for RadarTrip {
        connection,
        // route_name: _,
        route_type,
        route_color,
        segments,
    } in trips
    {
        {
            let TripSegment {
                from,
                to,
                departure_time,
                arrival_time,
            } = connection;
            let mut path = Path::begin_path();
            path.set_line_dash(&[2., 4.]);
            path.set_stroke_style(route_color);

            let mut to = to;
            if *from == geometry.geographic_origin {
                // connection is from origin meaning no natural bearing for it, we use the bearing to the next stop
                to = &segments.first().expect("at least one segment in a trip").to;
            }
            let (from_x, from_y, to_x, to_y) =
                geometry.line_coords(*from, *departure_time, *to, *arrival_time);
            path.move_to(to_x, to_y);
            path.line_to(from_x, from_y);
            paths.push(Box::new(path));
        }

        let mut path = Path::begin_path();
        use RouteType::*;
        if [
            Rail,
            RailwayService,
            SuburbanRailway,
            UrbanRailway,
            WaterTransportService,
        ]
        .contains(route_type)
        {
            path.set_line_width(2.);
        } else {
            // Bus, BusService, TramService,
            path.set_line_width(1.);
        }
        path.set_line_dash(&[]);
        path.set_stroke_style(&route_color);
        if segments.len() > 1 {
            let mut next_control_point = {
                // first segment
                let segment = &segments[0];
                let (from_x, from_y, to_x, to_y) = geometry.line_coords(
                    segment.from,
                    segment.departure_time,
                    segment.to,
                    segment.arrival_time,
                );
                path.move_to(from_x, from_y);
                let (cp1x, cp1y) = geometry.initial_control_point((from_x, from_y), (to_x, to_y));
                let (post_location, post_time) = if segments[1].from != segment.to {
                    (segments[1].from, segments[1].departure_time)
                } else {
                    (segments[1].to, segments[1].arrival_time)
                };
                let post_xy = geometry.coords(post_location, post_time);
                let ((cp2x, cp2y), next_control_point) =
                    geometry.control_points((from_x, from_y), (to_x, to_y), post_xy);
                path.bezier_curve_to(cp1x, cp1y, cp2x, cp2y, to_x, to_y);
                next_control_point
            };
            // all the connecting segments
            for window in segments.windows(3) {
                if let &[pre, segment, post] = &window {
                    let (from_x, from_y, to_x, to_y) = geometry.line_coords(
                        segment.from,
                        segment.departure_time,
                        segment.to,
                        segment.arrival_time,
                    );
                    let (pre_x, pre_y) =
                        geometry.coords(pre.to, pre.arrival_time);
                    if pre_x != from_x && pre_y != from_y {
                        // there is a gap in the route and so we move
                        path.move_to(from_x, from_y);
                    }
                    let (post_location, post_time) = if post.from != segment.to {
                        (post.from, post.departure_time)
                    } else {
                        (post.to, post.arrival_time)
                    };
                    let post_xy =
                        geometry.coords(post_location, post_time);
                    let (cp1x, cp1y) = next_control_point;
                    let ((cp2x, cp2y), next_control_point_tmp) =
                        geometry.control_points((from_x, from_y), (to_x, to_y), post_xy);
                    path.bezier_curve_to(cp1x, cp1y, cp2x, cp2y, to_x, to_y);
                    next_control_point = next_control_point_tmp;
                } else {
                    panic!("unusual window");
                }
            }
            {
                // draw the last curve
                let end = segments.last().unwrap();
                let (to_x, to_y) = geometry.coords(
                    end.from,
                    end.departure_time,
                );
                let (cp1x, cp1y) = next_control_point;
                path.bezier_curve_to(cp1x, cp1y, to_x, to_y, to_x, to_y);
            }
        } else {
            let segment = &segments[0];
            let (from_x, from_y, to_x, to_y) = geometry.line_coords(
                segment.from,
                segment.departure_time,
                segment.to,
                segment.arrival_time,
            );
            path.move_to(from_x, from_y);
            path.line_to(to_x, to_y);
        }
        paths.push(Box::new(path));
    }
    
    for Station {
        location,
        earliest_arrival,
        name,
    } in stations {
        const STOP_RADIUS: f64 = 3.;
        let (cx, cy) = geometry.coords(*location, *earliest_arrival);
        paths.push(Box::new(vec![
            Box::new(Circle::new(cx, cy, STOP_RADIUS)) as Box<dyn Drawable>,
            Box::new(Text::new(cx + STOP_RADIUS + 6., cy + 4., name.clone())),
        ]));
    }
    paths
}
