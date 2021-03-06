use radar_search::journey_graph;
use radar_search::search_data::*;
use radar_search::time::*;
use std::collections::HashMap;
use std::f64::consts::PI;

use super::canvasser;
use super::controls;

use canvasser::animate::*;
use canvasser::draw::*;

pub fn init(radar: Option<Radar>) -> canvasser::App<Option<Radar>, f64> {
    canvasser::App::new(should_draw, radar)
}

fn should_draw(model: &Option<Radar>, frame_count: u64, is_in_transition: bool) -> Option<f64> {
    if model.is_some() && (is_in_transition || frame_count % 6 == 0) {
        // @todo switch speed when something is transitioning maybe?
        let now = js_sys::Date::new_0();
        let now = ((now.get_hours() * 60 + now.get_minutes()) * 60 + now.get_seconds()) as f64
            + now.get_milliseconds() as f64 / 1000.;
        Some(now)
    } else {
        None
    }
}

pub struct Radar {
    pub geometry: Geo,
    trip_drawables: HashMap<TripId, Path<Polar>>,
    station_animatables: HashMap<StopId, Station<Polar>>,
    pub day: Day,
    pub expires_timestamp: u64,
    pub trip_count: usize,
}

#[derive(Clone)]
struct Station<G: Geometry> {
    coords: G::Coords,
    name: String,
}

struct RadarTrip {
    trip_id: TripId,
    #[allow(dead_code)]
    route_name: String,
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
pub struct Geo {
    pub start_time: Time,
    cartesian_origin: (f64, f64),
    geographic_origin: geo::Point<f64>,
    max_duration: Duration,
}

impl Geometry for Geo {
    type Coords = (geo::Point<f64>, Time);
}

impl Geo {
    fn bearing(&self, point: geo::Point<f64>) -> Option<Bearing> {
        if point == self.geographic_origin {
            None
        } else {
            Some(Bearing::degrees(geo::algorithm::bearing::Bearing::bearing(
                &self.geographic_origin,
                point,
            )))
        }
    }

    fn initial_control_point(
        &self,
        (start_point, start_time): (geo::Point<f64>, Time),
        (end_point, end_time): (geo::Point<f64>, Time),
    ) -> (Bearing, f64) {
        assert!(end_time >= start_time);

        // this is here because the calculation is not independent of start_time, this means that the animation will be an approximation and will distort :(
        let origin = self.start_time.seconds_since_midnight() as f64;

        let start_bearing = self.bearing(start_point);
        let end_bearing = self.bearing(end_point).unwrap();

        let start_mag = start_time.seconds_since_midnight() as f64 - origin;
        let end_mag = end_time.seconds_since_midnight() as f64 - origin;

        if let Some(start_bearing) = start_bearing {
            let bearing_difference = start_bearing.as_radians() - end_bearing.as_radians();
            let initial_heads_closer_to_origin =
                if bearing_difference >= PI / 2. || bearing_difference <= -PI / 2. {
                    // at PI or beyond PI/2 the tangent never crosses
                    true
                } else {
                    // compare the distance from origin of the end with the distance that the tangent to start would be at that bearing
                    end_mag < (start_mag / bearing_difference.cos())
                };

            // if a straight line between would travel closer to the origin than the start
            if initial_heads_closer_to_origin {
                // add a control point to prevent that
                let cp_bearing = start_bearing.as_radians() - bearing_difference / 3.;
                return (
                    Bearing::radians(cp_bearing),
                    origin + start_mag / (bearing_difference / 3.).cos(),
                );
            }
        }
        (start_bearing.unwrap_or(end_bearing), origin + start_mag)
    }

    fn control_points(
        &self,
        (bearing1, magnitude1): (Bearing, f64),
        (bearing2, magnitude2): (Bearing, f64),
        (bearing3, magnitude3): (Bearing, f64),
    ) -> ((Bearing, f64), (Bearing, f64)) {
        const CPFRAC: f64 = 0.3;
        // using a fake geometry to calculate these in cartsian space as I can't figure out the trigonometry from polar
        // what happens as the start time changes? does it skew weirdly?
        let polar = Polar::new(
            self.start_time.seconds_since_midnight() as f64,
            self.max_duration.to_secs() as f64,
            (0., 0.),
            self.cartesian_origin.0,
        );

        let (x1, y1) = polar.coords(bearing1, magnitude1);
        let (x2, y2) = polar.coords(bearing2, magnitude2);
        let (x3, y3) = polar.coords(bearing3, magnitude3);

        let angle_to_prev = (y2 - y1).atan2(x2 - x1);
        let angle_to_next = (y2 - y3).atan2(x2 - x3);
        // things to adjust and improve
        let angle_to_tangent = (PI + angle_to_next + angle_to_prev) / 2.;
        let cp2mag = -CPFRAC * ((x2 - x1).powi(2) + (y2 - y1).powi(2)).sqrt();
        let cp3mag = CPFRAC * ((x2 - x3).powi(2) + (y2 - y3).powi(2)).sqrt();
        // ^^ improve these
        let mut dx = angle_to_tangent.cos();
        let mut dy = angle_to_tangent.sin();
        if angle_to_prev < angle_to_next {
            dy = -dy;
            dx = -dx;
        }
        let (cp2_x, cp2_y) = (dx.mul_add(cp2mag, x2), dy.mul_add(cp2mag, y2));
        let (cp3_x, cp3_y) = (dx.mul_add(cp3mag, x2), dy.mul_add(cp3mag, y2));
        (
            // the need to negate here is weird, maybe the above atan2 calls are the wrong way around
            (
                Bearing::radians(-(cp2_y).atan2(cp2_x)),
                (cp2_x.powi(2) + cp2_y.powi(2)).sqrt() * self.max_duration.to_secs() as f64
                    / self.cartesian_origin.0
                    + self.start_time.seconds_since_midnight() as f64,
            ),
            (
                Bearing::radians(-(cp3_y).atan2(cp3_x)),
                (cp3_x.powi(2) + cp3_y.powi(2)).sqrt() * self.max_duration.to_secs() as f64
                    / self.cartesian_origin.0
                    + self.start_time.seconds_since_midnight() as f64,
            ),
        )
    }
}

pub fn day_time(date_time: &js_sys::Date) -> (Day, Time) {
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

pub fn search(data: &GTFSData, origin: &Stop, controls: &controls::Params) -> Radar {
    // TODO don't use client time, instead start with the server time and increment using client clock, also this is local time
    let (day, start_time) = day_time(&js_sys::Date::new_0());
    let max_duration = Duration::minutes(30);
    let end_time = start_time + max_duration;
    let max_extra_search = Duration::minutes(10);
    let mut plotter = journey_graph::Plotter::new(
        day,
        Period::between(start_time, end_time + max_extra_search),
        data,
    );
    plotter.add_origin_station(origin);
    if controls.flags.show_sbahn {
        plotter.add_route_type(RouteType::SuburbanRailway)
    }
    if controls.flags.show_ubahn {
        plotter.add_route_type(RouteType::UrbanRailway)
    }
    if controls.flags.show_bus {
        plotter.add_route_type(RouteType::BusService);
        plotter.add_route_type(RouteType::Bus)
    }
    if controls.flags.show_tram {
        plotter.add_route_type(RouteType::TramService)
    }
    if controls.flags.show_regional {
        plotter.add_route_type(RouteType::RailwayService)
    }
    let mut expires_time = end_time;
    let mut trips: HashMap<TripId, RadarTrip> = HashMap::new();

    let mut station_animatables: HashMap<StopId, Station<Polar>> = HashMap::new();
    let mut trip_drawables: HashMap<TripId, Path<Polar>> = HashMap::new();
    let geometry = Geo {
        cartesian_origin: (500., 500.),
        geographic_origin: origin.location,
        start_time,
        max_duration,
    };

    for item in plotter {
        match item {
            journey_graph::Item::Station {
                stop,
                earliest_arrival,
            } => {
                if earliest_arrival > end_time + (expires_time - start_time) {
                    break;
                }
                let station = Station {
                    coords: (stop.location, earliest_arrival),
                    name: stop.stop_name.replace(" (Berlin)", ""),
                };
                assert!(station_animatables
                    .insert(stop.station_id(), station.into_polar(&geometry))
                    .is_none());
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
                        trip_id,
                        route_name: route_name.to_string(),
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

    for RadarTrip {
        trip_id,
        connection,
        route_name: _,
        route_type,
        route_color,
        segments,
    } in trips.values()
    {
        use RouteType::{
            Rail, RailwayService, SuburbanRailway, UrbanRailway, WaterTransportService,
        };
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
            if *to == geometry.geographic_origin {
                // connection is on origin meaning no natural bearing for it, we use the bearing to the next stop
                to = &segments.first().expect("at least one segment in a trip").to;
            }
            // done in reverse, it means the line moves into the origin rather than just erasing from the end
            path.move_to((
                geometry.bearing(*to).unwrap(),
                arrival_time.seconds_since_midnight() as f64,
            ));
            path.line_to((
                geometry.bearing(*from).unwrap_or_default(),
                departure_time.seconds_since_midnight() as f64,
            ));
            // use random to make connection and trip unique in hashmap
            trip_drawables.insert(1_000_000 + *trip_id, path);
        }

        let mut path = Path::begin_path();
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
        path.set_stroke_style(route_color);
        match segments.len().cmp(&1) {
            std::cmp::Ordering::Greater => {
                let mut next_control_point = {
                    // first segment
                    let segment = &segments[0];
                    let to_bearing = geometry.bearing(segment.to).unwrap();
                    let from_bearing = geometry.bearing(segment.from).unwrap_or(to_bearing);
                    let from_mag = segment.departure_time.seconds_since_midnight() as f64;
                    let to_mag = segment.arrival_time.seconds_since_midnight() as f64;
                    path.move_to((from_bearing, from_mag));

                    let cp1 = geometry.initial_control_point(
                        (segment.from, segment.departure_time),
                        (segment.to, segment.arrival_time),
                    );
                    let (post_location, post_time) = if segments[1].from == segment.to {
                        (segments[1].to, segments[1].arrival_time)
                    } else {
                        (segments[1].from, segments[1].departure_time)
                    };
                    let post_bearing = geometry.bearing(post_location).unwrap();
                    let post_mag = post_time.seconds_since_midnight() as f64;

                    let (cp2, next_control_point) = geometry.control_points(
                        (from_bearing, from_mag),
                        (to_bearing, to_mag),
                        (post_bearing, post_mag),
                    );
                    path.bezier_curve_to(cp1, cp2, (to_bearing, to_mag));
                    next_control_point
                };
                // all the connecting segments
                for window in segments.windows(3) {
                    if let [pre, segment, post] = window {
                        let to_bearing = geometry.bearing(segment.to).unwrap();
                        let from_bearing = geometry.bearing(segment.from).unwrap_or(to_bearing);
                        let from_mag = segment.departure_time.seconds_since_midnight() as f64;
                        let to_mag = segment.arrival_time.seconds_since_midnight() as f64;

                        let pre_bearing = geometry.bearing(pre.to).unwrap();
                        let pre_mag = pre.arrival_time.seconds_since_midnight() as f64;
                        if pre_bearing != from_bearing && (pre_mag - from_mag).abs() > f64::EPSILON
                        {
                            // there is a gap in the route and so we move
                            path.move_to((from_bearing, from_mag));
                            next_control_point = geometry.initial_control_point(
                                (segment.from, segment.departure_time),
                                (segment.to, segment.arrival_time),
                            );
                        }
                        let (post_location, post_time) = if post.from == segment.to {
                            (post.to, post.arrival_time)
                        } else {
                            (post.from, post.departure_time)
                        };
                        let post_bearing = geometry.bearing(post_location).unwrap();
                        let post_mag = post_time.seconds_since_midnight() as f64;
                        let cp1 = next_control_point;
                        let (cp2, next_control_point_tmp) = geometry.control_points(
                            (from_bearing, from_mag),
                            (to_bearing, to_mag),
                            (post_bearing, post_mag),
                        );
                        path.bezier_curve_to(cp1, cp2, (to_bearing, to_mag));
                        next_control_point = next_control_point_tmp;
                    } else {
                        panic!("unusual window");
                    }
                }
                {
                    // draw the last curve
                    let end = segments.last().unwrap();
                    let to = (
                        geometry.bearing(end.to).unwrap(),
                        end.arrival_time.seconds_since_midnight() as f64,
                    );
                    let cp1 = next_control_point;
                    path.bezier_curve_to(cp1, to, to);
                }
            }
            std::cmp::Ordering::Equal => {
                let segment = &segments[0];
                let to_bearing = geometry.bearing(segment.to).unwrap();
                let from_bearing = geometry.bearing(segment.from).unwrap_or(to_bearing);

                path.move_to((
                    from_bearing,
                    segment.departure_time.seconds_since_midnight() as f64,
                ));
                path.line_to((
                    to_bearing,
                    segment.arrival_time.seconds_since_midnight() as f64,
                ));
            }
            std::cmp::Ordering::Less => {
                // path is empty - ignore
            }
        }
        trip_drawables.insert(*trip_id, path);
    }

    let expires_timestamp = js_sys::Date::new_0();
    expires_timestamp.set_hours(expires_time.hour() as u32);
    expires_timestamp.set_minutes(expires_time.minute() as u32);
    expires_timestamp.set_seconds(expires_time.second() as u32 + 1); // expire once this second is over
    expires_timestamp.set_milliseconds(0);
    // trip_drawables.reverse();
    // station_animatables.reverse();

    Radar {
        day,
        expires_timestamp: expires_timestamp.value_of() as u64,
        geometry,
        trip_drawables,
        station_animatables,
        trip_count: trips.len(),
    }
}

impl Drawable for Geo {
    fn draw(&self, ctx: &web_sys::CanvasRenderingContext2d, _: &Cartesian) {
        let (origin_x, origin_y) = self.cartesian_origin;
        ctx.set_line_dash(&js_sys::Array::of2(&10_f64.into(), &10_f64.into()).into())
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

#[derive(Default)]
pub struct TransitionCtx {
    stations: HashMap<StopId, CartesianTransitionContext>,
    trips: HashMap<TripId, PathTransitionContext>,
}

impl TransitionContext for TransitionCtx {
    fn is_in_transition(&self) -> bool {
        self.stations
            .values()
            .any(TransitionContext::is_in_transition)
    }
}

impl Animatable<f64> for Radar {
    type TransitionContext = TransitionCtx;

    fn draw_frame(
        &self,
        day_millis: &f64,
        transition_context: &mut TransitionCtx,
        canvas: &web_sys::CanvasRenderingContext2d,
        _: &Cartesian,
    ) {
        let Self {
            day: _,
            expires_timestamp: _,
            geometry,
            station_animatables,
            trip_drawables,
            trip_count: _,
        } = self;

        geometry.draw(canvas, &Cartesian);
        let polar_geometry = Polar::new(
            *day_millis,
            geometry.max_duration.to_secs() as f64,
            geometry.cartesian_origin,
            f64::min(geometry.cartesian_origin.0, geometry.cartesian_origin.1),
        );
        trip_drawables.draw_frame(
            day_millis,
            &mut transition_context.trips,
            canvas,
            &polar_geometry,
        );
        station_animatables.draw_frame(
            day_millis,
            &mut transition_context.stations,
            canvas,
            &polar_geometry,
        );
    }
}

impl Drawable for Station<Cartesian> {
    fn draw(&self, ctx: &web_sys::CanvasRenderingContext2d, _: &Cartesian) {
        const STOP_RADIUS: f64 = 3.;
        let (cx, cy) = self.coords;
        Circle::new((cx, cy), STOP_RADIUS).draw(ctx, &Cartesian);
        Text::new(cx + STOP_RADIUS + 6., cy + 4., self.name.clone()).draw(ctx, &Cartesian);
    }
}

impl Station<Geo> {
    fn into_polar(self, geometry: &Geo) -> Station<Polar> {
        let (point, time) = self.coords;
        Station::<Polar> {
            coords: (
                geometry.bearing(point).unwrap_or_default(),
                time.seconds_since_midnight() as f64,
            ),
            name: self.name,
        }
    }
}

impl Drawable<Polar> for Station<Polar> {
    fn draw(&self, ctx: &web_sys::CanvasRenderingContext2d, geometry: &Polar) {
        const STOP_RADIUS: f64 = 3.;
        let (bearing, magnitude) = self.coords;
        if magnitude > geometry.max() {
            return;
        }
        let (cx, cy) = geometry.coords(bearing, magnitude);
        Circle::new((cx, cy), STOP_RADIUS).draw(ctx, &Cartesian);
        Text::new(cx + STOP_RADIUS + 6., cy + 4., self.name.clone()).draw(ctx, &Cartesian);
    }
}

impl Animatable<f64, Polar> for Station<Polar> {
    type TransitionContext = CartesianTransitionContext;

    fn draw_frame(
        &self,
        time: &f64,
        transition_ctx: &mut CartesianTransitionContext,
        canvas: &web_sys::CanvasRenderingContext2d,
        geometry: &Polar,
    ) {
        const STOP_RADIUS: f64 = 3.;
        let (bearing, magnitude) = self.coords;
        if magnitude > geometry.max() {
            // not drawing so remove the context too
            *transition_ctx = CartesianTransitionContext::None;
            return;
        }

        // set the target
        let new_target = geometry.coords(bearing, magnitude);
        // position to acutally draw
        let (cx, cy) = transition_ctx
            .or_start(geometry.coords(bearing, geometry.max()))
            .process_transition_frame(new_target, *time, 1.);

        Circle::new((cx, cy), STOP_RADIUS).draw(canvas, &Cartesian);
        Text::new(cx + STOP_RADIUS + 6., cy + 4., self.name.clone()).draw(canvas, &Cartesian);
    }
}
