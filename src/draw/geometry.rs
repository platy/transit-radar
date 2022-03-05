use std::{fmt, io, ops};

use chrono::{DateTime, Duration};
use chrono_tz::Tz;

use crate::write_xml;

// Not sure if being generic over geometries makes sense anymore
pub trait Geometry {
    type Coords;
}

pub struct Cartesian;

impl Geometry for Cartesian {
    type Coords = (Pixels, Pixels);
}

/// TODO Not sure if this should be an f64
#[derive(Clone, Copy)]
pub struct Pixels(f64);

impl ops::Mul<Pixels> for f64 {
    type Output = Pixels;

    fn mul(self, rhs: Pixels) -> Self::Output {
        Pixels(self * rhs.0)
    }
}

impl ops::Div<Pixels> for f64 {
    type Output = f64;

    fn div(self, rhs: Pixels) -> Self::Output {
        self / rhs.0
    }
}

impl ops::Sub for Pixels {
    type Output = Pixels;

    fn sub(self, rhs: Self) -> Self::Output {
        Pixels(self.0 - rhs.0)
    }
}
impl Pixels {
    pub fn new(val: f64) -> Self {
        Self(val)
    }
    pub(crate) fn atan2(&self, other: Pixels) -> f64 {
        self.0.atan2(other.0)
    }
}
impl ops::Deref for Pixels {
    type Target = f64;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl fmt::Display for Pixels {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.1}", self.0)
    }
}

/// A geometry which represents each point as a polar coordinate, a bearing and a time. The geometry has an origin time, the point of the cone.
pub struct FlattenedTimeCone {
    origin: DateTime<Tz>,
    max_duration: Duration,
    max_points: Pixels, // maybe replace with scale, something like points per minute
}

impl FlattenedTimeCone {
    pub const fn new(
        origin: DateTime<Tz>,
        max_duration: Duration,
        max_points: Pixels, // maybe replace with scale, something like points per minute
    ) -> Self {
        Self {
            origin,
            max_duration,
            max_points,
        }
    }

    pub fn coords(&self, bearing: Bearing, magnitude: DateTime<Tz>) -> (Pixels, Pixels) {
        let radius = magnitude - self.origin;
        if radius < Duration::zero() {
            (Pixels(0.), Pixels(0.))
        } else {
            let h = radius.num_seconds() as f64 / self.max_duration.num_seconds() as f64;
            let x = h * bearing.as_radians().cos();
            let y = h * bearing.as_radians().sin();
            (x * self.max_points, (-y) * self.max_points)
        }
    }

    pub fn max(&self) -> DateTime<Tz> {
        self.origin + self.max_duration
    }

    pub fn max_duration(&self) -> Duration {
        self.max_duration
    }

    pub fn max_points(&self) -> Pixels {
        self.max_points
    }

    pub(crate) fn origin(&self) -> DateTime<Tz> {
        self.origin
    }
}

#[derive(Default, PartialEq, Copy, Clone)]
pub struct Bearing(f64);

impl Bearing {
    pub const fn radians(radians: f64) -> Self {
        Self(radians)
    }

    pub fn degrees(degrees: f64) -> Self {
        use std::f64::consts::PI;
        Self(degrees * PI / 180.)
    }

    pub const fn as_radians(self) -> f64 {
        self.0
    }
}

impl std::fmt::Debug for Bearing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)?;
        f.write_str(" rad")?;
        Ok(())
    }
}

impl Geometry for FlattenedTimeCone {
    type Coords = (Bearing, DateTime<Tz>);
}

pub struct Path<G: Geometry> {
    pub line_width: f64,
    pub line_dash: Vec<f64>,
    pub stroke_style: Option<String>,
    pub ops: Vec<PathTo<G>>,
}

pub enum PathTo<G: Geometry> {
    Move(G::Coords),
    Line(G::Coords),
    BezierCurve(G::Coords, G::Coords, G::Coords),
}

impl<G: Geometry> Path<G> {
    pub fn begin_path() -> Self {
        Self {
            line_width: 1.,
            line_dash: vec![],
            stroke_style: None,
            ops: vec![],
        }
    }

    pub fn set_line_width(&mut self, line_width: f64) {
        self.line_width = line_width;
    }

    pub fn set_line_dash(&mut self, line_dash: &[f64]) {
        self.line_dash = line_dash.into();
    }

    pub fn set_stroke_style(&mut self, stroke_style: String) {
        self.stroke_style = Some(stroke_style);
    }

    pub fn move_to(&mut self, coords: G::Coords) {
        self.ops.push(PathTo::Move(coords));
    }

    pub fn line_to(&mut self, coords: G::Coords) {
        self.ops.push(PathTo::Line(coords));
    }

    pub fn bezier_curve_to(&mut self, cp1: G::Coords, cp2: G::Coords, to: G::Coords) {
        self.ops.push(PathTo::BezierCurve(cp1, cp2, to))
    }
}

struct DisplayInGeometry<T, G> {
    display: T,
    geometry: G,
}

impl<T, I> std::fmt::Display for DisplayInGeometry<T, &FlattenedTimeCone>
where
    T: for<'a> IntoIterator<Item = I> + Copy,
    I: std::borrow::Borrow<PathTo<FlattenedTimeCone>>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for item in self.display {
            match *item.borrow() {
                PathTo::Move((bearing, magnitude)) => {
                    if magnitude > self.geometry.max() {
                        return Ok(());
                    }
                    let (x, y) = self.geometry.coords(bearing, magnitude);
                    write!(f, "M {} {} ", x, y)?;
                }
                PathTo::Line((bearing, magnitude)) => {
                    if magnitude > self.geometry.max() {
                        return Ok(());
                    }
                    let (x, y) = self.geometry.coords(bearing, magnitude);
                    write!(f, "{} {} ", x, y)?;
                }
                PathTo::BezierCurve(
                    (cp1_bearing, cp1_magnitude),
                    (cp2_bearing, cp2_magnitude),
                    (bearing, magnitude),
                ) => {
                    if magnitude > self.geometry.max() {
                        return Ok(());
                    }
                    let (cp1_x, cp1_y) = self.geometry.coords(cp1_bearing, cp1_magnitude);
                    let (cp2_x, cp2_y) = self.geometry.coords(cp2_bearing, cp2_magnitude);
                    let (x, y) = self.geometry.coords(bearing, magnitude);
                    write!(f, "C {} {} {} {} {} {} ", cp1_x, cp1_y, cp2_x, cp2_y, x, y)?;
                }
            }
        }
        Ok(())
    }
}

impl Path<FlattenedTimeCone> {
    pub(crate) fn write_svg_fragment_to(
        &self,
        w: &mut dyn io::Write,
        geometry: &FlattenedTimeCone,
    ) -> io::Result<()> {
        write_xml!(w,
            <path
                stroke-width={self.line_width}
                stroke={self.stroke_style.as_deref().unwrap_or("black")}
                fill={"none"}
                stroke-dasharray=[&self.line_dash,]
                d={DisplayInGeometry { display: &self.ops, geometry }} />)
    }
}
