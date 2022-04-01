use chrono::prelude::*;
use chrono::Duration;
use chrono_tz::Tz;
use radar_search::journey_graph;
use radar_search::search_data::*;
use radar_search::time::*;
use std::borrow::Cow;
use std::collections::HashMap;
use std::collections::HashSet;
use std::f64::consts::PI;
use std::fmt::Display;
use std::fmt::Write;
use std::io;

use crate::write_xml;

use super::geometry::*;

pub struct Radar<'s> {
    geometry: Geo,
    trips: HashMap<TripId, RadarTrip<'s>>,
    stations: HashMap<StopId, Station<'s, FlattenedTimeCone>>,
    origin: &'s Stop,
}

struct Station<'s, G: Geometry> {
    coords: G::Coords,
    stop: &'s Stop,
    name_trunk_length: usize,
}

#[derive(Debug)]
struct RadarTrip<'s> {
    _trip_id: TripId,
    route_name: String,
    route_type: RouteType,
    /// Usually just one of these, each item is a connection into this trip and the segments that follow it
    parts: Vec<(TripSegment<'s>, Vec<TripSegment<'s>>)>,
}

#[derive(Debug)]
struct TripSegment<'s> {
    from: &'s Stop,
    to: &'s Stop,
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
        let origin = self.time_cone_geometry.origin();

        let start_bearing = self.bearing(start_point);
        let end_bearing = self.bearing(end_point).unwrap();

        initial_control_point(origin, (start_bearing, start_time), (end_bearing, end_time))
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

fn initial_control_point(
    origin: DateTime<Tz>,
    (start_bearing, start_time): (Option<Bearing>, Time),
    (end_bearing, end_time): (Bearing, Time),
) -> (Bearing, DateTime<Tz>) {
    let origin_secs = origin.time().num_seconds_from_midnight() as f64;

    let start_mag = start_time.seconds_since_midnight() as f64 - origin_secs;
    let end_mag = end_time.seconds_since_midnight() as f64 - origin_secs;

    assert!(
        start_mag > -60.,
        "starting magnitude is less than zero {} : {:?} - {:?}",
        start_mag,
        start_time,
        origin.time()
    );
    if let Some(start_bearing) = start_bearing {
        let bearing_difference = (start_bearing - end_bearing).normalize_around_zero();
        let initial_heads_closer_to_origin = if bearing_difference.as_radians().abs() >= PI / 2. {
            // at PI or beyond PI/2 the tangent never crosses
            true
        } else {
            // compare the distance from origin of the end with the distance that the tangent to start would be at that bearing
            end_mag < (start_mag / bearing_difference.as_radians().cos())
        };

        // if a straight line between would travel closer to the origin than the start
        if initial_heads_closer_to_origin {
            // add a control point to prevent that
            let cp_bearing_difference = bearing_difference.as_radians() / 3.;
            let cp_bearing = start_bearing.as_radians() - cp_bearing_difference;
            let mag = origin + Duration::seconds((start_mag / cp_bearing_difference.cos()) as i64);
            assert!(
                mag > origin,
                "initial_control_point({:?}, {:?}) at origin {}",
                (start_bearing, start_time),
                (end_bearing, end_time),
                origin
            );
            return (Bearing::radians(cp_bearing), mag);
        }
    }
    let mag = origin + Duration::seconds(start_mag as i64);
    assert!(mag >= origin, "{} {}", mag, origin);
    (start_bearing.unwrap_or(end_bearing), mag)
}

#[test]
fn sane_initial_control_point() {
    let origin = FixedOffset::east(3600)
        .ymd(2020, 1, 1)
        .and_hms(0, 0, 0)
        .with_timezone(&chrono_tz::Europe::Berlin);
    for &((start_bearing, start_seconds), (end_bearing, end_seconds)) in &[
        ((None, 0), (Bearing::degrees(90.), 300)),
        (
            (Some(Bearing::radians(-3.045)), 20 * 60),
            (Bearing::radians(3.0196), 22 * 60),
        ),
    ] {
        let (_bearing, mag) = initial_control_point(
            origin,
            (
                start_bearing,
                Time::from_seconds_since_midnight(start_seconds),
            ),
            (end_bearing, Time::from_seconds_since_midnight(end_seconds)),
        );
        let start_time = NaiveTime::from_num_seconds_from_midnight(start_seconds, 0);
        let end_time = NaiveTime::from_num_seconds_from_midnight(end_seconds, 0);
        let actual_time = mag.time();
        assert!(
            start_time <= actual_time && actual_time < end_time,
            "Expected {} < time < {}, but time was {}",
            start_time,
            end_time,
            actual_time
        );
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum TransitMode {
    SBahn,
    UBahn,
    Bus,
    Tram,
    Regional,
    Boat,
}

impl TransitMode {
    const DEFAULTS: &'static [TransitMode] = &[TransitMode::SBahn, TransitMode::UBahn];

    fn key(&self) -> &str {
        match self {
            TransitMode::SBahn => "sbahn",
            TransitMode::UBahn => "ubahn",
            TransitMode::Bus => "bus",
            TransitMode::Tram => "tram",
            TransitMode::Regional => "regional",
            TransitMode::Boat => "boat",
        }
    }
}

impl Display for TransitMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransitMode::SBahn => f.write_str("S-Bahn"),
            TransitMode::UBahn => f.write_str("U-Bahn"),
            TransitMode::Bus => f.write_str("Bus"),
            TransitMode::Tram => f.write_str("Tram"),
            TransitMode::Regional => f.write_str("Regional"),
            TransitMode::Boat => f.write_str("Boat"),
        }
    }
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
pub struct SearchParams<'s> {
    pub origin: &'s Stop,
    pub departure_time: Option<DateTime<Tz>>,
    pub max_duration: Duration,
    pub modes: Cow<'s, HashSet<TransitMode>>,
}

#[derive(Debug, Clone)]
pub struct UrlSearchParams<'s> {
    pub station_id: StopId,
    pub departure_time: Option<DateTime<Tz>>,
    pub max_duration: Duration,
    pub modes: Cow<'s, HashSet<TransitMode>>,
}

impl<'s> UrlSearchParams<'s> {
    fn with_station_id(self, station_id: StopId) -> Self {
        Self {
            station_id,
            departure_time: self.departure_time,
            max_duration: self.max_duration,
            modes: self.modes,
        }
    }

    fn with_departure_time(self, departure_time: DateTime<Tz>) -> Self {
        Self {
            station_id: self.station_id,
            departure_time: Some(departure_time),
            max_duration: self.max_duration,
            modes: self.modes,
        }
    }

    fn with_mode(self, mode: TransitMode) -> Self {
        let mut modes = self.modes.into_owned();
        modes.insert(mode);
        Self {
            station_id: self.station_id,
            departure_time: self.departure_time,
            max_duration: self.max_duration,
            modes: Cow::Owned(modes),
        }
    }

    fn without_mode(self, mode: TransitMode) -> Self {
        let mut modes = self.modes.into_owned();
        modes.remove(&mode);
        Self {
            station_id: self.station_id,
            departure_time: self.departure_time,
            max_duration: self.max_duration,
            modes: Cow::Owned(modes),
        }
    }
}

pub const STATION_ID_MIN: u64 = 900_000_000_000;
pub const DEFAULT_MAX_DURATION_MINS: i64 = 30;

impl<'s> Display for UrlSearchParams<'s> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "/depart-from/{}/",
            self.station_id.get() - STATION_ID_MIN,
        )?;
        if let Some(time) = self.departure_time {
            write!(f, "{:?}", time.naive_local())?;
        } else {
            f.write_str("now")?;
        }
        let uses_non_default_minutes = self.max_duration.num_minutes() != DEFAULT_MAX_DURATION_MINS;
        let uses_non_default_modes = *self.modes != TransitMode::DEFAULTS.iter().copied().collect();
        if uses_non_default_minutes || uses_non_default_modes {
            f.write_char('?')?;
        }
        if uses_non_default_minutes {
            write!(f, "minutes={}", self.max_duration.num_minutes())?;
        }
        if uses_non_default_minutes && uses_non_default_modes {
            write!(f, "&amp;")?;
        }
        if *self.modes != TransitMode::DEFAULTS.iter().copied().collect() {
            write!(f, "mode=")?;
            let mut iter = self.modes.iter().peekable();
            while let Some(mode) = iter.next() {
                f.write_str(mode.key())?;
                if iter.peek().is_some() {
                    write!(f, ",")?;
                }
            }
        }
        Ok(())
    }
}

pub fn search<'s>(
    data: &'s GTFSData,
    SearchParams {
        origin,
        departure_time,
        max_duration,
        modes,
    }: SearchParams<'s>,
) -> Radar<'s> {
    let departure_time =
        departure_time.unwrap_or_else(|| Utc::now().with_timezone(&chrono_tz::Europe::Berlin));
    let (day, start_time) = day_time(departure_time);
    let end_time = start_time + max_duration;
    let max_extra_search = Duration::minutes(0);
    let mut plotter = journey_graph::Plotter::new(
        day,
        Period::between(start_time, end_time + max_extra_search),
        data,
    );
    plotter.add_origin_station(origin);
    if modes.contains(&TransitMode::SBahn) {
        plotter.add_route_type(RouteType::SuburbanRailway);
    }
    if modes.contains(&TransitMode::UBahn) {
        plotter.add_route_type(RouteType::UrbanRailway);
    }
    if modes.contains(&TransitMode::Bus) {
        plotter.add_route_type(RouteType::BusService);
        plotter.add_route_type(RouteType::Bus);
    }
    if modes.contains(&TransitMode::Tram) {
        plotter.add_route_type(RouteType::TramService);
    }
    if modes.contains(&TransitMode::Regional) {
        plotter.add_route_type(RouteType::RailwayService);
        plotter.add_route_type(RouteType::Rail);
    }
    if modes.contains(&TransitMode::Boat) {
        plotter.add_route_type(RouteType::WaterTransportService);
    }
    let mut expires_time = end_time;
    let mut trips: HashMap<TripId, RadarTrip> = HashMap::new();

    let mut stations: HashMap<StopId, Station<FlattenedTimeCone>> = HashMap::new();
    let geometry = Geo {
        time_cone_geometry: FlattenedTimeCone::new(departure_time, max_duration, Pixels::new(500.)),
        geographic_origin: origin.location,
    };

    for item in plotter {
        match item {
            journey_graph::Item::Station {
                stop,
                earliest_arrival,
                name_trunk_length,
            } => {
                if earliest_arrival > end_time + (expires_time - start_time) {
                    break;
                }
                let station = Station {
                    coords: (stop.location, earliest_arrival),
                    stop,
                    name_trunk_length: if name_trunk_length == stop.stop_name.len() {
                        continue;
                    } else if name_trunk_length > 10 {
                        // last space before the common chars end
                        let trunk_division = stop
                            .stop_name
                            .chars()
                            .enumerate()
                            .filter_map(|(i, c)| {
                                (c.is_whitespace() && i < name_trunk_length).then(|| i + 1)
                            })
                            .last()
                            .unwrap_or_default();
                        trunk_division
                    } else {
                        0
                    },
                };
                assert!(stations
                    .insert(stop.station_id(), station.into_polar(&geometry))
                    .is_none());
            }
            journey_graph::Item::Transfer {
                departure_time: _,
                arrival_time: _,
                from_stop: _,
                to_stop: _,
            } => {
                // eprintln!("Ignoring transfer from {:?} to {:?}", from_stop, to_stop);
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
                let segment = TripSegment {
                    from: from_stop,
                    to: to_stop,
                    departure_time,
                    arrival_time,
                };
                if let Some(pre) = trip.parts.last().unwrap().1.last() {
                    assert!(
                        !(pre.to != segment.from && pre.arrival_time != segment.departure_time),
                        "shouldn't have a gap in the route : from ({:?} {}) to ({:?} {})",
                        pre.to,
                        pre.arrival_time,
                        segment.from,
                        segment.departure_time,
                    );
                }
                trip.parts.last_mut().unwrap().1.push(segment);
            }
            journey_graph::Item::ConnectionToTrip {
                departure_time,
                arrival_time,
                from_stop,
                to_stop,
                trip_id,
                route_name,
                route_type,
                route_color: _,
            } => {
                let adjusted_departure_time = stations
                    .get(&from_stop.station_id())
                    .map(|station| station.coords.1.time().into())
                    .unwrap_or(departure_time);
                if adjusted_departure_time != departure_time {
                    eprintln!("Had to adjust departure time of connection to {}({}) from {:?} as the departure time was {} but {:?} is reached earliest at {}",
                        route_name, trip_id, from_stop, departure_time, from_stop, adjusted_departure_time,
                    );
                }

                trips
                    .entry(trip_id)
                    .or_insert_with(|| RadarTrip {
                        _trip_id: trip_id,
                        route_name: route_name.to_string(),
                        route_type,
                        parts: Vec::with_capacity(1),
                    })
                    .parts
                    .push((
                        TripSegment {
                            from: from_stop,
                            to: to_stop,
                            departure_time: adjusted_departure_time,
                            arrival_time,
                        },
                        vec![],
                    ));
            }
        }
    }

    Radar {
        origin,
        geometry,
        trips,
        stations,
    }
}

impl<'s> RadarTrip<'s> {
    pub(crate) fn write_svg_fragment_to(
        &self,
        w: &mut dyn std::io::Write,
        geometry: &Geo,
    ) -> io::Result<()> {
        let RadarTrip {
            _trip_id: _,
            route_name,
            route_type,
            parts,
        } = self;
        let time_to_datetime = |time: Time| {
            geometry
                .time_cone_geometry
                .origin()
                .date()
                .and_time(time.into())
                .unwrap()
        };
        for (connection, segments) in parts {
            // At Wannsee, bus 118 leaves Wannsee and arrives at Wannsee 2 minutes later according to my data, remove any of these
            let mut segments = &segments[..];
            for i in 0..segments.len() {
                if segments[i].to.location != geometry.geographic_origin {
                    segments = &segments[i..];
                    break;
                }
            }
            {
                let TripSegment {
                    from,
                    to,
                    departure_time,
                    arrival_time,
                } = connection;
                let mut path = Path::begin_path();
                path.set_class(format!("Connection {:?} {}", route_type, route_name));

                let mut to = to;
                if to.location == geometry.geographic_origin {
                    // connection is on origin meaning no natural bearing for it, we use the bearing to the next stop
                    to = &segments.first().expect("at least one segment in a trip").to;
                }
                path.move_to((
                    geometry.bearing(from.location).unwrap_or_default(),
                    time_to_datetime(*departure_time),
                ));
                path.line_to((
                    geometry.bearing(to.location).unwrap(),
                    time_to_datetime(*arrival_time),
                ));
                path.write_svg_fragment_to(w, &geometry.time_cone_geometry)?;
            }

            let mut path = Path::begin_path();
            path.set_class(format!("{:?} {}", route_type, route_name));
            match segments.len().cmp(&1) {
                std::cmp::Ordering::Greater => {
                    let mut next_control_point = {
                        // first segment
                        let segment = &segments[0];
                        let to_bearing = geometry
                            .bearing(segment.to.location)
                            .expect("Segment goes to origin");
                        let from_bearing = geometry
                            .bearing(segment.from.location)
                            .unwrap_or(to_bearing);
                        let from_mag = time_to_datetime(segment.departure_time);
                        let to_mag = time_to_datetime(segment.arrival_time);
                        path.move_to((from_bearing, from_mag));

                        let cp1 = geometry.initial_control_point(
                            (segment.from.location, segment.departure_time),
                            (segment.to.location, segment.arrival_time),
                        );
                        assert!(cp1.1 >= from_mag, "Control point for curve cannot have a lower magnitude than the origin, {} must be > {}", cp1.1, from_mag);
                        let (post_stop, post_time) = if segments[1].from == segment.to {
                            (segments[1].to, segments[1].arrival_time)
                        } else {
                            (segments[1].from, segments[1].departure_time)
                        };
                        let post_bearing = geometry
                            .bearing(post_stop.location)
                            .expect("Segment goes to origin");
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
                            assert!(
                                !(pre.to != segment.from
                                    && pre.arrival_time != segment.departure_time),
                                "shouldn't have a gap in the route : from ({:?} {}) to ({:?} {})",
                                pre.to,
                                pre.arrival_time,
                                segment.from,
                                segment.departure_time,
                            );

                            let to_bearing = geometry.bearing(segment.to.location).unwrap();
                            let from_bearing = geometry
                                .bearing(segment.from.location)
                                .unwrap_or(to_bearing);
                            let from_mag = segment.departure_time;
                            let to_mag = segment.arrival_time;

                            let (post_stop, post_time) = if post.from == segment.to {
                                (post.to, post.arrival_time)
                            } else {
                                (post.from, post.departure_time)
                            };
                            let post_bearing = geometry.bearing(post_stop.location).unwrap();
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
                            geometry.bearing(end.to.location).unwrap(),
                            time_to_datetime(end.arrival_time),
                        );
                        let cp1 = next_control_point;
                        path.bezier_curve_to(cp1, to, to);
                    }
                }
                std::cmp::Ordering::Equal => {
                    let segment = &segments[0];
                    let to_bearing = geometry.bearing(segment.to.location).unwrap();
                    let from_bearing = geometry
                        .bearing(segment.from.location)
                        .unwrap_or(to_bearing);

                    path.move_to((from_bearing, time_to_datetime(segment.departure_time)));
                    path.line_to((to_bearing, time_to_datetime(segment.arrival_time)));
                }
                std::cmp::Ordering::Less => {
                    panic!("path is empty - ignore");
                }
            }
            assert!(!path.ops.is_empty());
            path.write_svg_fragment_to(w, &geometry.time_cone_geometry)?;
        }
        Ok(())
    }
}

impl Geo {
    fn write_svg_fragment_to(&self, w: &mut dyn io::Write) -> io::Result<()> {
        let (origin_x, origin_y) = (0., 0.);

        const PIXEL_RADIUS: f64 = 500.;
        let max_duration = self.time_cone_geometry.max_duration();
        let duration_interval = if max_duration <= Duration::minutes(20) {
            Duration::minutes(5)
        } else {
            Duration::minutes(10)
        };
        let pixel_interval: f64 = PIXEL_RADIUS * duration_interval.num_seconds() as f64
            / max_duration.num_seconds() as f64;

        write_xml!(w,
            <g class="grid">)?;

        for radius in (1..)
            .map(|x| pixel_interval * x as f64)
            .take_while(|p: &f64| p <= &PIXEL_RADIUS)
        {
            write_xml!(w, <circle cx={origin_x} cy={origin_y} r={radius} />)?;
        }
        write_xml!(w, </g>)?;

        Ok(())
    }
}

impl<'s> Radar<'s> {
    pub fn write_svg_to(
        &self,
        w: &mut dyn io::Write,
        search_params: UrlSearchParams<'s>,
        refresh: bool,
    ) -> io::Result<()> {
        let Self {
            geometry,
            stations,
            trips,
            origin,
        } = self;

        writeln!(
            w,
            r#"<!DOCTYPE svg PUBLIC "-//W3C//DTD SVG 1.1//EN" "http://www.w3.org/Graphics/SVG/1.1/DTD/svg11.dtd">
<svg version="1.1" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="100%" height="100%" viewBox="-512 -512 1024 1024">
    <title>{} departures: Transit Radar</title>
    <desc>Departure tree.</desc>
         "#,
            origin.stop_name
        )?;

        write_xml!(w, <style>{include_str!("Radar.css")}</style>)?;

        write_xml!(w,
            <g id="header" transform="translate(-506, -506)">
                <text y="20" style="font-size: 20pt;">{origin.stop_name}{" departures"}</text>
                <a href={search_params.clone().with_departure_time(geometry.time_cone_geometry.origin())} rel="self"><text y="50" style="font-size: 10pt; font-style: oblique;">
                    "All trips starting "{geometry.time_cone_geometry.origin().format("at %k:%M on %e %b %Y")}
                    <tspan x="0" dy="1.4em">{"and lasting less than "}{geometry.time_cone_geometry.max_duration().num_minutes()}{" minutes"}</tspan>
                </text></a>
                <text id="refresh-notice" y="90" visibility="hidden">"refreshing every 5 seconds [disable]"</text>
                <text y="110" id="transport-types">
        )?;
        for &mode in &[
            TransitMode::SBahn,
            TransitMode::UBahn,
            TransitMode::Tram,
            TransitMode::Bus,
            TransitMode::Regional,
            TransitMode::Boat,
        ] {
            let mode_enabled = search_params.modes.contains(&mode);
            write_xml!(w,
                <tspan x="0" dy="1.5em" class={ if mode_enabled { "" } else {"disabled"}}>
                    <a href={if mode_enabled {
                        search_params.clone().without_mode(mode)
                    } else {
                        search_params.clone().with_mode(mode)
                    }}>{mode}</a>
                </tspan>
            )?;
        }
        write_xml!(w,
                </text>
                <text id="credit" y="200"><a href="https://radar.njk.onl">"from transit radar,"</a><tspan x="0" dy="1.4em" ><a href="mailto:platy@njk.lonl">"by platy"</a></tspan></text>
            </g>
        )?;

        geometry.write_svg_fragment_to(w)?;
        for trip in trips.values() {
            trip.write_svg_fragment_to(w, geometry)?;
        }
        write_xml!(w, <g class="s">)?;
        for station in stations.values() {
            station.write_svg_fragment_to(w, &geometry.time_cone_geometry, &search_params)?;
        }
        write_xml!(w, </g>)?;

        if refresh {
            write_xml!(w,
                <script>{r#"
                const refreshTimeout = setTimeout(() => location.reload(), 5000);
                const refreshNotice = document.getElementById('refresh-notice')
                refreshNotice.setAttribute('visibility', 'visible');
                refreshNotice.onclick = () => {
                    clearTimeout(refreshTimeout);
                    refreshNotice.setAttribute('visibility', 'hidden');
                }
                "#}</script>)?;
        }

        writeln!(w, "</svg>")
    }
}

impl<'s> Station<'s, Geo> {
    fn into_polar(self, geometry: &Geo) -> Station<'s, FlattenedTimeCone> {
        let (point, time) = self.coords;
        Station::<'s, FlattenedTimeCone> {
            coords: (
                geometry.bearing(point).unwrap_or_default(),
                geometry
                    .time_cone_geometry
                    .origin()
                    .date()
                    .and_time(time.into())
                    .unwrap(),
            ),
            stop: self.stop,
            name_trunk_length: self.name_trunk_length,
        }
    }
}

impl<'s> Station<'s, FlattenedTimeCone> {
    pub(crate) fn write_svg_fragment_to(
        &self,
        w: &mut dyn io::Write,
        geometry: &FlattenedTimeCone,
        search_params: &UrlSearchParams,
    ) -> io::Result<()> {
        const STOP_RADIUS: f64 = 3.;
        let (bearing, magnitude) = self.coords;
        if magnitude > geometry.max() {
            return Ok(());
        }
        let (cx, cy) = geometry.coords(bearing, magnitude);
        let name: std::borrow::Cow<_> = if self.name_trunk_length == 0 {
            (*self.stop.stop_name).into()
        } else {
            format!("...{}", &self.stop.stop_name[self.name_trunk_length..]).into()
        };
        write_xml!(w,
            <a href={search_params.clone().with_station_id(self.stop.station_id())}>
            <circle cx={*cx} cy={*cy} r={STOP_RADIUS} />
                <text x={*cx + STOP_RADIUS + 6.} y={*cy + 4.}>{name}</text>
            </a>
        )?;
        Ok(())
    }
}
