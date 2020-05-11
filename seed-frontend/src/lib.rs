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

mod canvasser;

#[wasm_bindgen(start)]
pub fn render() {
    App::builder(update, view)
        .after_mount(after_mount)
        .build_and_start();
}

fn after_mount(_: Url, orders: &mut impl Orders<Msg>) -> AfterMount<Model> {
    orders.send_msg(Msg::SyncMsg(sync::Msg::FetchData));
    AfterMount::default()
}

#[derive(Default)]
struct Model {
    sync: sync::Model<GTFSData>,
    canvas: ElRef<HtmlCanvasElement>,
    canvas_scaled: Option<f64>,
    radar: Option<Radar>,

    controls: controls::Model,
}

enum Msg {
    SyncMsg(sync::Msg<GTFSData, GTFSSyncIncrement>),
    DataUpdated,
    Draw,
    ControlsMsg(controls::Msg),
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
        Msg::SyncMsg(msg) => {
            let query = serde_urlencoded::to_string(Params {
                ubahn: model.controls.show_ubahn,
                sbahn: model.controls.show_sbahn,
                bus: model.controls.show_bus,
                tram: model.controls.show_tram,
                regio: model.controls.show_regional,
            }).unwrap();
            let url = format!("/data/U%20Voltastr.%20(Berlin)?{}", query);
            if sync::update(msg, &mut model.sync, url, &mut orders.proxy(Msg::SyncMsg)) {
                orders.send_msg(Msg::DataUpdated);
            }
        }

        Msg::DataUpdated => {
            model.radar = Some(search(model.sync.get().unwrap(), &model.controls));
            orders.after_next_render(|_| Msg::Draw);
        }

        Msg::Draw => {
            if let &mut Some(ref mut radar) = &mut model.radar {
                let date = js_sys::Date::new_0();
                let (_day, time) = day_time(&date);
                if date.value_of() <= radar.expires_timestamp {
                    radar.geometry.start_time = time;
                } else {
                    model.radar = Some(search(model.sync.get().unwrap(), &model.controls));
                    orders.send_msg(Msg::SyncMsg(sync::Msg::FetchData));
                }
            } else {
                model.radar = Some(search(model.sync.get().unwrap(), &model.controls));
            }
            draw(model);
            // The next time a render is triggered we will draw again
            orders.after_next_render(|_| Msg::Draw);
            if !model.controls.animate {
                orders.skip();
            }
        }

        Msg::ControlsMsg(msg) => {
            controls::update(msg, &mut model.controls, &mut orders.proxy(Msg::ControlsMsg));
            model.radar = Some(search(model.sync.get().unwrap(), &model.controls));
            orders.after_next_render(|_| Msg::SyncMsg(sync::Msg::FetchData));
            orders.after_next_render(|_| Msg::Draw);
        }
    }
}

fn view(model: &Model) -> Node<Msg> {
    if let Some(data) = model.sync.get() {
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

fn search(data: &GTFSData, controls: &controls::Model) -> Radar {
    // TODO don't use client time, instead start with the server time and increment using client clock, also this is local time
    let (day, start_time) = day_time(&js_sys::Date::new_0());
    let max_duration = Duration::minutes(30);
    let mut plotter = journey_graph::JourneyGraphPlotter::new(day, Period::between(start_time, start_time + max_duration), &data);
    let origin = data.get_stop(&900000007103).unwrap();
    plotter.add_origin_station(origin);
    if controls.show_sbahn { plotter.add_route_type(RouteType::SuburbanRailway) }
    if controls.show_ubahn { plotter.add_route_type(RouteType::UrbanRailway) }
    if controls.show_bus { plotter.add_route_type(RouteType::BusService); plotter.add_route_type(RouteType::Bus) }
    if controls.show_tram { plotter.add_route_type(RouteType::TramService) }
    if controls.show_regional { plotter.add_route_type(RouteType::RailwayService) }
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

use canvasser::{Drawable, Path};

impl Drawable for RadarGeometry {
    fn draw(&self, ctx: &web_sys::CanvasRenderingContext2d) {
        let (origin_x, origin_y) = self.cartesian_origin;
        ctx.set_line_dash(&js_sys::Array::of2(&10f64.into(), &10f64.into()).into()).unwrap();
        ctx.set_stroke_style(&"lightgray".into());
        ctx.set_line_width(1.);
        ctx.begin_path();
        ctx.arc(origin_x, origin_y, 500. / 3., 0., 2. * std::f64::consts::PI).unwrap();
        ctx.stroke();
        ctx.begin_path();
        ctx.arc(origin_x, origin_y, 500. * 2. / 3., 0., 2. * std::f64::consts::PI).unwrap();
        ctx.stroke();
        ctx.begin_path();
        ctx.arc(origin_x, origin_y, 500., 0., 2. * std::f64::consts::PI).unwrap();
        ctx.stroke();
        ctx.set_line_dash(&js_sys::Array::new().into()).unwrap();
    }
}

fn radar_view(Radar { day: _, expires_timestamp: _, geometry, trips, }: &Radar, data: &GTFSData) -> impl Drawable {
    let mut paths = vec![];
    for RadarTrip { connection, route_name: _, route_type, route_color, segments } in trips {
        {
            let TripSegment { from, to, departure_time, arrival_time } = connection;
            let mut path = Path::begin_path();
            path.set_line_dash(&[2., 4.]);
            path.set_stroke_style(route_color);
            
            let start_point = data.get_stop(from).unwrap().location;
            let mut end_point = data.get_stop(to).unwrap().location;
            if end_point == geometry.geographic_origin {
                // connection is from origin meaning no natural bearing for it, we use the bearing to the next stop
                end_point = data.get_stop(&segments.first().expect("at least one segment in a trip").to).unwrap().location;
            } 
            let (from_x, from_y, to_x, to_y) = geometry.line_coords(start_point, *departure_time, end_point, *arrival_time);
            path.move_to(to_x, to_y);
            path.line_to(from_x, from_y);
            paths.push(path);
        }

        let mut path = Path::begin_path();
        use RouteType::*;
        if [Rail, RailwayService, SuburbanRailway, UrbanRailway, WaterTransportService].contains(route_type) {
            path.set_line_width(2.);
        } else { // Bus, BusService, TramService,
            path.set_line_width(1.);
        }
        path.set_line_dash(&[]);
        path.set_stroke_style(&route_color);
        if segments.len() > 1 {
            let mut next_control_point = { // first segment
                let segment = &segments[0];
                let (from_x, from_y, to_x, to_y) = geometry.line_coords(data.get_stop(&segment.from).unwrap().location, segment.departure_time, data.get_stop(&segment.to).unwrap().location, segment.arrival_time);
                path.move_to(from_x, from_y);
                let (cp1x, cp1y) = geometry.initial_control_point((from_x, from_y), (to_x, to_y));
                let (post_id, post_time) = if segments[1].from != segment.to {
                    (segments[1].from, segments[1].departure_time)
                } else {
                    (segments[1].to, segments[1].arrival_time)
                };
                let post_xy = geometry.coords(data.get_stop(&post_id).unwrap().location, post_time);
                let ((cp2x, cp2y), next_control_point) = geometry.control_points((from_x, from_y), (to_x, to_y), post_xy);
                path.bezier_curve_to(cp1x, cp1y, cp2x, cp2y, to_x, to_y);
                next_control_point
            };
            // all the connecting segments
            for window in segments.windows(3) {
                if let &[pre, segment, post] = &window {
                    let (from_x, from_y, to_x, to_y) = geometry.line_coords(data.get_stop(&segment.from).unwrap().location, segment.departure_time, data.get_stop(&segment.to).unwrap().location, segment.arrival_time);
                    let (pre_x, pre_y) = geometry.coords(data.get_stop(&pre.to).unwrap().location, pre.arrival_time);
                    if pre_x != from_x && pre_y != from_y {
                        // there is a gap in the route and so we move
                        path.move_to(from_x, from_y);
                    }
                    let (post_id, post_time) = if post.from != segment.to {
                        (post.from, post.departure_time)
                    } else {
                        (post.to, post.arrival_time)
                    };
                    let post_xy = geometry.coords(data.get_stop(&post_id).unwrap().location, post_time);
                    let (cp1x, cp1y) = next_control_point;
                    let ((cp2x, cp2y), next_control_point_tmp) = geometry.control_points((from_x, from_y), (to_x, to_y), post_xy);
                    path.bezier_curve_to(cp1x, cp1y, cp2x, cp2y, to_x, to_y);
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
                path.bezier_curve_to(cp1x, cp1y, to_x, to_y, to_x, to_y);
            }
        } else {
            let segment = &segments[0];
            let (from_x, from_y, to_x, to_y) = geometry.line_coords(data.get_stop(&segment.from).unwrap().location, segment.departure_time, data.get_stop(&segment.to).unwrap().location, segment.arrival_time);
            path.move_to(from_x, from_y);
            path.line_to(to_x, to_y);
        }
        paths.push(path);
    }
    paths
}

fn draw(model: &mut Model) { // todo , error type to encapsulate all the kinds of error in draw
    let canvas = model.canvas.get().expect("get canvas element");
    let ctx = seed::canvas_context_2d(&canvas);
    ctx.set_global_composite_operation("source-over").unwrap()  ;
    ctx.clear_rect(0., 0., 1200., 1000.);

    if model.canvas_scaled.is_none() {
        let scale = 2.; // expects scaling of 2, should be fine if the scaling is 1 also
        ctx.scale(scale, scale).unwrap();
        model.canvas_scaled = Some(scale);
    }

    if let Some(radar) = &model.radar {
        radar.geometry.draw(&ctx);

        let data = model.sync.get().unwrap();

        let paths = radar_view(radar, data);
        paths.draw(&ctx);
    }
}


/// Data fetching module which fetches required data based on requirements from the client, the server can send increments of the data required to meet the requirements.
mod sync {
    use seed::{prelude::*, fetch, error};
    use serde::{Serialize, de::DeserializeOwned};
    use radar_search::naive_sync::SyncData;
    use futures::prelude::*;

    pub enum Msg<D, I> {
        DataFetched(Result<SyncData<D, I>, LoadError>),
        FetchData,
    }


    pub struct Model<D> {
        status: RequestStatus,
        sync: ServerSync<D>,
    }

    impl<D> Default for Model<D> {
        fn default() -> Model<D> {
            Model {
                status: RequestStatus::Ready,
                sync: ServerSync::NotSynced,
            }
        }
    }

    enum RequestStatus {
        /// no request is being made
        Ready,
        /// a request is being made
        InProgress,
        /// a request is being made and another request is needed
        Invalidated,
    }

    pub enum ServerSync<D> {
        NotSynced,
        Synced {
            session_id: u64,
            update_count: u64,
            data: D
        }
    }

    pub fn update<D: 'static, I: 'static, Gm: 'static>(msg: Msg<D, I>, model: &mut Model<D>, url: String, orders: &mut impl Orders<Msg<D, I>, Gm>) -> bool
    where D: std::ops::AddAssign<I> + DeserializeOwned,
          I: DeserializeOwned {
        match msg {
            Msg::FetchData => {
                match model.status {
                    RequestStatus::Ready => {
                        orders.perform_cmd(request(model.url(url)).map(Msg::<D, I>::DataFetched));
                        orders.skip();
                        model.status = RequestStatus::InProgress;
                    }
                    _ => {
                        model.status = RequestStatus::Invalidated;
                    }
                }
                false
            }
    
            Msg::DataFetched(Ok(data)) => {
                model.receive(data);
                match model.status {
                    RequestStatus::Ready => {
                        panic!("unexpected response data");
                    }
                    RequestStatus::InProgress => {
                        model.status = RequestStatus::Ready;
                    }
                    RequestStatus::Invalidated => {
                        model.status = RequestStatus::Ready;
                        orders.send_msg(Msg::FetchData);
                    }
                }
                true
            },
    
            Msg::DataFetched(Err(fail_reason)) => {
                error!(format!(
                    "Fetch error - Fetching repository info failed - {:#?}",
                    fail_reason
                ));
                orders.skip();
                false
            },
        }
    }

    async fn request<S>(url: String) -> Result<S, LoadError>
    where S: DeserializeOwned {
        let response = fetch::fetch(url).await?;
        let body = response.bytes().await?;
        Ok(rmp_serde::from_read_ref(&body)?)
    }

    impl<D> Model<D> {
        /// todo use a header instead and leave the url to the caller
        pub fn url(&self, mut url: String) -> String {
            if let ServerSync::Synced { session_id, update_count, data: _ } = self.sync {
                let query = serde_urlencoded::to_string(SyncParams {
                    id: session_id,
                    count: update_count,
                }).unwrap();
                url += "&";
                url += &query;
            }
            url
        }

        // todo check update numbers
        pub fn receive<'de, I>(&mut self, sync_data: SyncData<D, I>) -> &D
        where D: std::ops::AddAssign<I> {

            match sync_data {
                SyncData::Initial {
                    session_id,
                    update_number: update_count,
                    data,
                } => {
                    self.sync = ServerSync::Synced {
                        session_id,
                        update_count,
                        data,
                    };
                    self.get().unwrap()
                }

                SyncData::Increment {
                    increment,
                    update_number,
                    session_id,
                } => {
                    if let ServerSync::Synced { 
                        session_id: our_session_id,
                        update_count,
                        data: existing_data, 
                    } = &mut self.sync {
                        *existing_data += increment;
                        *update_count = update_number;
                        assert!(session_id == *our_session_id, "session ids don't match");
                        &*existing_data
                    } else {
                        panic!("bad sync: retrieved increment with no data locally");
                    }
                }
            }
        }

        pub fn get(&self) -> Option<&D> {
            match &self.sync {
                ServerSync::NotSynced =>  None,
                ServerSync::Synced {
                    data,
                    update_count: _,
                    session_id: _,
                } => Some(data),
            }
        }
    }

    #[derive(Serialize)]
    struct SyncParams {
        id: u64,
        count: u64,
    }

    #[derive(Debug)]
    pub enum LoadError {
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
}

mod controls {
    use seed::{*, prelude::*};

    #[derive(Default, Clone)]
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
            checkbox("show-regional", "Show Regional", model.show_regional, &Msg::SetShowRegional),
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
