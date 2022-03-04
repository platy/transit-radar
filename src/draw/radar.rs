use chrono::Duration;
use chrono::prelude::*;
use chrono_tz::Tz;
use radar_search::journey_graph;
use radar_search::search_data::*;
use radar_search::time::*;
use std::collections::HashMap;
use std::f64::consts::PI;
use std::io;
use std::num::NonZeroU32;

use crate::write_xml_element;

use super::geometry::*;

pub struct Radar {
    geometry: Geo,
    trips: HashMap<TripId, Path<FlattenedTimeCone>>,
    stations: HashMap<StopId, Station<FlattenedTimeCone>>,
}

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

// Geographical flattened time cone geometry, the bearing is calculated from an origin position.
pub struct Geo {
    time_cone_geometry: FlattenedTimeCone,
    geographic_origin: geo::Point<f64>,
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
    ) -> (Bearing, DateTime<Tz>) {
        assert!(end_time >= start_time);

        // this is here because the calculation is not independent of start_time, this means that the animation will be an approximation and will distort :(
        let origin = self
            .time_cone_geometry
            .origin()
            .time()
            .num_seconds_from_midnight() as f64;

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
                    self.time_cone_geometry.origin()
                        + Duration::seconds((start_mag / (bearing_difference / 3.).cos()) as i64),
                );
            }
        }
        (
            start_bearing.unwrap_or(end_bearing),
            self.time_cone_geometry.origin() + Duration::seconds(start_mag as i64),
        )
    }

    fn control_points(
        &self,
        (bearing1, magnitude1): (Bearing, DateTime<Tz>),
        (bearing2, magnitude2): (Bearing, DateTime<Tz>),
        (bearing3, magnitude3): (Bearing, DateTime<Tz>),
    ) -> ((Bearing, DateTime<Tz>), (Bearing, DateTime<Tz>)) {
        const CPFRAC: f64 = 0.3;
        // calculate these in cartesian space as I can't figure out the trigonometry from polar
        let polar = &self.time_cone_geometry;

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
        let (cp2_x, cp2_y) = (dx.mul_add(cp2mag, *x2), dy.mul_add(cp2mag, *y2));
        let (cp3_x, cp3_y) = (dx.mul_add(cp3mag, *x2), dy.mul_add(cp3mag, *y2));

        (
            // the need to negate here is weird, maybe the above atan2 calls are the wrong way around
            (
                Bearing::radians(-(cp2_y).atan2(cp2_x)),
                polar.origin()
                    + Duration::milliseconds(
                        (polar.max_duration().num_milliseconds() as f64
                            * ((cp2_x.powi(2) + cp2_y.powi(2)).sqrt() / polar.max_points()))
                            as i64,
                    ),
            ),
            (
                Bearing::radians(-(cp3_y).atan2(cp3_x)),
                polar.origin()
                    + Duration::milliseconds(
                        (polar.max_duration().num_milliseconds() as f64
                            * ((cp3_x.powi(2) + cp3_y.powi(2)).sqrt() / polar.max_points()))
                            as i64,
                    ),
            ),
        )
    }
}

#[derive(Default, Debug, Clone, Eq, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
pub struct Flags {
    pub show_sbahn: bool,
    pub show_ubahn: bool,
    pub show_bus: bool,
    pub show_tram: bool,
    pub show_regional: bool,
}

pub fn day_time<Tz: TimeZone>(date_time: DateTime<Tz>) -> (Day, Time) {
    let now = Time::from_seconds_since_midnight(date_time.num_seconds_from_midnight());
    let day = match date_time.weekday() {
        chrono::Weekday::Mon => Day::Monday,
        chrono::Weekday::Tue => Day::Tuesday,
        chrono::Weekday::Wed => Day::Wednesday,
        chrono::Weekday::Thu => Day::Thursday,
        chrono::Weekday::Fri => Day::Friday,
        chrono::Weekday::Sat => Day::Saturday,
        chrono::Weekday::Sun => Day::Sunday,
    };
    (day, now)
}

pub fn search(data: &GTFSData, origin: &Stop, flags: &Flags) -> Radar {
    let departure_time = Utc::now().with_timezone(&chrono_tz::Europe::Berlin);
    let departure_date = departure_time.date();
    let (day, start_time) = day_time(departure_time);
    let max_duration = Duration::minutes(30);
    let end_time = start_time + max_duration;
    let max_extra_search = Duration::minutes(10);
    let mut plotter = journey_graph::Plotter::new(
        day,
        Period::between(start_time, end_time + max_extra_search),
        data,
    );
    plotter.add_origin_station(origin);
    if flags.show_sbahn {
        plotter.add_route_type(RouteType::SuburbanRailway)
    }
    if flags.show_ubahn {
        plotter.add_route_type(RouteType::UrbanRailway)
    }
    if flags.show_bus {
        plotter.add_route_type(RouteType::BusService);
        plotter.add_route_type(RouteType::Bus)
    }
    if flags.show_tram {
        plotter.add_route_type(RouteType::TramService)
    }
    if flags.show_regional {
        plotter.add_route_type(RouteType::RailwayService)
    }
    let mut expires_time = end_time;
    let mut trips: HashMap<TripId, RadarTrip> = HashMap::new();

    let mut station_animatables: HashMap<StopId, Station<FlattenedTimeCone>> = HashMap::new();
    let mut trip_drawables: HashMap<TripId, Path<FlattenedTimeCone>> = HashMap::new();
    let geometry = Geo {
        time_cone_geometry: FlattenedTimeCone::new(
            departure_time,
            max_duration,
            Pixels::new(500.),
        ),
        geographic_origin: origin.location,
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
        let time_to_datetime = |time: Time| departure_date.and_time(time.into()).unwrap();
        {
            let TripSegment {
                from,
                to,
                departure_time,
                arrival_time,
            } = connection;
            let mut path = Path::begin_path();
            path.set_line_dash(&[2., 4.]);
            path.set_stroke_style(route_color.clone());

            let mut to = to;
            if *to == geometry.geographic_origin {
                // connection is on origin meaning no natural bearing for it, we use the bearing to the next stop
                to = &segments.first().expect("at least one segment in a trip").to;
            }
            // done in reverse, it means the line moves into the origin rather than just erasing from the end
            path.move_to((
                geometry.bearing(*to).unwrap(),
                time_to_datetime(*arrival_time),
            ));
            path.line_to((
                geometry.bearing(*from).unwrap_or_default(),
                time_to_datetime(*departure_time),
            ));
            // use random to make connection and trip unique in hashmap
            trip_drawables.insert(NonZeroU32::new(1_000_000 + trip_id.get()).unwrap(), path);
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
        path.set_stroke_style(route_color.clone());
        match segments.len().cmp(&1) {
            std::cmp::Ordering::Greater => {
                let mut next_control_point = {
                    // first segment
                    let segment = &segments[0];
                    let to_bearing = geometry.bearing(segment.to).unwrap();
                    let from_bearing = geometry.bearing(segment.from).unwrap_or(to_bearing);
                    let from_mag = time_to_datetime(segment.departure_time);
                    let to_mag = time_to_datetime(segment.arrival_time);
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
                    let post_mag = time_to_datetime(post_time);

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
                        let from_mag = segment.departure_time;
                        let to_mag = segment.arrival_time;

                        let pre_bearing = geometry.bearing(pre.to).unwrap();
                        let pre_mag = pre.arrival_time;
                        if pre_bearing != from_bearing && pre_mag != from_mag {
                            // there is a gap in the route and so we move
                            path.move_to((from_bearing, time_to_datetime(from_mag)));
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
                        let post_mag = post_time;
                        let cp1 = next_control_point;
                        let (cp2, next_control_point_tmp) = geometry.control_points(
                            (from_bearing, time_to_datetime(from_mag)),
                            (to_bearing, time_to_datetime(to_mag)),
                            (post_bearing, time_to_datetime(post_mag)),
                        );
                        path.bezier_curve_to(cp1, cp2, (to_bearing, time_to_datetime(to_mag)));
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
                        time_to_datetime(end.arrival_time),
                    );
                    let cp1 = next_control_point;
                    path.bezier_curve_to(cp1, to, to);
                }
            }
            std::cmp::Ordering::Equal => {
                let segment = &segments[0];
                let to_bearing = geometry.bearing(segment.to).unwrap();
                let from_bearing = geometry.bearing(segment.from).unwrap_or(to_bearing);

                path.move_to((from_bearing, time_to_datetime(segment.departure_time)));
                path.line_to((to_bearing, time_to_datetime(segment.arrival_time)));
            }
            std::cmp::Ordering::Less => {
                // path is empty - ignore
            }
        }
        trip_drawables.insert(*trip_id, path);
    }

    Radar {
        geometry,
        trips: trip_drawables,
        stations: station_animatables,
    }
}



impl Geo {
    fn write_svg_fragment_to(&self, w: &mut dyn io::Write) -> io::Result<()> {
        let (origin_x, origin_y) = (0., 0.);

        write!(
            w,
            r#"<g stroke-dasharray="10,10" stroke="lightgray" stroke-width="1" fill="none">"#
        )?;
        write_xml_element!(w, <circle cx={origin_x} cy={origin_y} r={500. / 3.} />)?;
        write_xml_element!(w, <circle cx={origin_x} cy={origin_y} r={500. * 2. / 3.} />)?;
        write_xml_element!(w, <circle cx={origin_x} cy={origin_y} r={500} />)?;
        write!(w, r#"</g>"#)?;

        Ok(())
    }
}

impl Radar {
    pub fn write_svg_to(&self, w: &mut dyn io::Write) -> io::Result<()> {
        let Self {
            geometry,
            stations: station_animatables,
            trips: trip_drawables,
        } = self;

        writeln!(
            w,
            r#"<!DOCTYPE svg PUBLIC "-//W3C//DTD SVG 1.1//EN" "http://www.w3.org/Graphics/SVG/1.1/DTD/svg11.dtd">
<svg version="1.1" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="100%" height="100%" viewBox="-512 -512 1024 1024">
    <title>Transit Radar</title>
    <desc>Departure tree.</desc>
         "#
        )?;

        geometry.write_svg_fragment_to(w)?;
        for trip in trip_drawables.values() {
            trip.write_svg_fragment_to(w, &geometry.time_cone_geometry)?;
        }
        for station in station_animatables.values() {
            station.write_svg_fragment_to(w, &geometry.time_cone_geometry)?;
        }
        writeln!(w, "</svg>")
    }
}

// impl Drawable for Station<Cartesian> {
//     fn draw(&self, ctx: &web_sys::CanvasRenderingContext2d, _: &Cartesian) {
//         const STOP_RADIUS: f64 = 3.;
//         let (cx, cy) = self.coords;
//         Circle::new((cx, cy), STOP_RADIUS).draw(ctx, &Cartesian);
//         Text::new(cx + STOP_RADIUS + 6., cy + 4., self.name.clone()).draw(ctx, &Cartesian);
//     }
// }

impl Station<Geo> {
    fn into_polar(self, geometry: &Geo) -> Station<FlattenedTimeCone> {
        let (point, time) = self.coords;
        Station::<FlattenedTimeCone> {
            coords: (
                geometry.bearing(point).unwrap_or_default(),
                geometry
                    .time_cone_geometry
                    .origin()
                    .date()
                    .and_time(time.into())
                    .unwrap(),
            ),
            name: self.name,
        }
    }
}

impl Station<FlattenedTimeCone> {
    pub(crate) fn write_svg_fragment_to(
        &self,
        w: &mut dyn io::Write,
        geometry: &FlattenedTimeCone,
    ) -> io::Result<()> {
        const STOP_RADIUS: f64 = 3.;
        let (bearing, magnitude) = self.coords;
        if magnitude > geometry.max() {
            return Ok(());
        }
        let (cx, cy) = geometry.coords(bearing, magnitude);
        write_xml_element!(w, <circle cx={*cx} cy={*cy} r={STOP_RADIUS} />)?;
        writeln!(
            w,
            r#"<text x="{:.1}" y="{:.1}">{}</text>"#,
            *cx + STOP_RADIUS + 6.,
            *cy + 4.,
            self.name
        )?;
        Ok(())
    }
}
