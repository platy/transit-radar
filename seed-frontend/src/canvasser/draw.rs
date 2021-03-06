use std::iter::FromIterator;
use wasm_bindgen::JsValue;
use web_sys::CanvasRenderingContext2d;

pub trait Geometry {
    type Coords;
}

pub struct Cartesian;

impl Geometry for Cartesian {
    type Coords = (f64, f64);
}

pub struct Polar {
    origin: f64,
    max: f64,
    cartesian_offset: (f64, f64),
    cartesian_max: f64,
}

impl Polar {
    pub const fn new(
        origin: f64,
        max: f64,
        cartesian_offset: (f64, f64),
        cartesian_max: f64,
    ) -> Self {
        Self {
            origin,
            max,
            cartesian_offset,
            cartesian_max,
        }
    }

    pub fn coords(&self, bearing: Bearing, magnitude: f64) -> (f64, f64) {
        let radius = magnitude - self.origin;
        if radius < 0. {
            self.cartesian_offset
        } else {
            let h = radius / self.max;
            let x = h * bearing.as_radians().cos();
            let y = h * bearing.as_radians().sin();
            (
                x.mul_add(self.cartesian_max, self.cartesian_offset.0),
                (-y).mul_add(self.cartesian_max, self.cartesian_offset.1),
            )
        }
    }

    pub fn max(&self) -> f64 {
        self.origin + self.max
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

impl Geometry for Polar {
    type Coords = (Bearing, f64);
}

pub trait Drawable<G = Cartesian>
where
    Self: Sized,
{
    fn draw(&self, canvas: &CanvasRenderingContext2d, geometry: &G);
    fn as_cartesian(self, geometry: G) -> AsCartesian<G, Self> {
        AsCartesian {
            shape: self,
            geometry,
        }
    }
}

impl<T, G> Drawable<G> for Vec<T>
where
    T: Drawable<G>,
{
    fn draw(&self, ctx: &CanvasRenderingContext2d, geometry: &G) {
        for i in self {
            i.draw(ctx, geometry);
        }
    }
}

impl<T, G> Drawable<G> for Option<T>
where
    T: Drawable<G>,
{
    fn draw(&self, ctx: &CanvasRenderingContext2d, geometry: &G) {
        for i in self {
            i.draw(ctx, geometry);
        }
    }
}

impl<T: ?Sized, G> Drawable<G> for Box<T>
where
    T: Drawable<G>,
{
    fn draw(&self, ctx: &CanvasRenderingContext2d, geometry: &G) {
        self.as_ref().draw(ctx, geometry);
    }
}

pub struct AsCartesian<G, D: Drawable<G>> {
    pub shape: D,
    pub geometry: G,
}

impl<G, D: Drawable<G>> Drawable<Cartesian> for AsCartesian<G, D> {
    fn draw(&self, canvas: &CanvasRenderingContext2d, _geometry: &Cartesian) {
        self.shape.draw(canvas, &self.geometry)
    }
}

pub struct Path<G: Geometry> {
    pub line_width: f64,
    pub line_dash: Vec<f64>,
    pub stroke_style: Option<String>,
    pub ops: Vec<PathOp<G>>,
}

#[allow(clippy::pub_enum_variant_names)]
pub enum PathOp<G: Geometry> {
    MoveTo(G::Coords),
    LineTo(G::Coords),
    BezierCurveTo(G::Coords, G::Coords, G::Coords),
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

    // todo cow
    pub fn set_stroke_style(&mut self, stroke_style: &str) {
        self.stroke_style = Some(stroke_style.to_owned());
    }

    pub fn move_to(&mut self, coords: G::Coords) {
        self.ops.push(PathOp::MoveTo(coords));
    }

    pub fn line_to(&mut self, coords: G::Coords) {
        self.ops.push(PathOp::LineTo(coords));
    }

    pub fn bezier_curve_to(&mut self, cp1: G::Coords, cp2: G::Coords, to: G::Coords) {
        self.ops.push(PathOp::BezierCurveTo(cp1, cp2, to))
    }
}

impl Drawable for Path<Cartesian> {
    fn draw(&self, ctx: &CanvasRenderingContext2d, _: &Cartesian) {
        ctx.begin_path();
        ctx.set_line_width(self.line_width);
        ctx.set_line_dash(&js_sys::Array::from_iter(
            self.line_dash.iter().cloned().map(JsValue::from_f64),
        ))
        .unwrap();
        if let Some(stroke_style) = &self.stroke_style {
            ctx.set_stroke_style(&JsValue::from_str(stroke_style));
        }

        for op in &self.ops {
            match *op {
                PathOp::MoveTo((x, y)) => ctx.move_to(x, y),
                PathOp::LineTo((x, y)) => ctx.line_to(x, y),
                PathOp::BezierCurveTo((cp1_x, cp1_y), (cp2_x, cp2_y), (x, y)) => {
                    ctx.bezier_curve_to(cp1_x, cp1_y, cp2_x, cp2_y, x, y)
                }
            }
        }
        if self.stroke_style.is_some() {
            ctx.stroke();
        }
    }
}

impl Drawable<Polar> for Path<Polar> {
    fn draw(&self, ctx: &CanvasRenderingContext2d, geometry: &Polar) {
        ctx.begin_path();
        ctx.set_line_width(self.line_width);
        ctx.set_line_dash(&js_sys::Array::from_iter(
            self.line_dash.iter().cloned().map(JsValue::from_f64),
        ))
        .unwrap();
        if let Some(stroke_style) = &self.stroke_style {
            ctx.set_stroke_style(&JsValue::from_str(stroke_style));
        }

        for op in &self.ops {
            match *op {
                PathOp::MoveTo((bearing, magnitude)) => {
                    if magnitude > geometry.max() {
                        break;
                    }
                    let (x, y) = geometry.coords(bearing, magnitude);
                    ctx.move_to(x, y)
                }
                PathOp::LineTo((bearing, magnitude)) => {
                    if magnitude > geometry.max() {
                        break;
                    }
                    let (x, y) = geometry.coords(bearing, magnitude);
                    ctx.line_to(x, y)
                }
                PathOp::BezierCurveTo(
                    (cp1_bearing, cp1_magnitude),
                    (cp2_bearing, cp2_magnitude),
                    (bearing, magnitude),
                ) => {
                    if magnitude > geometry.max() {
                        break;
                    }
                    let (cp1_x, cp1_y) = geometry.coords(cp1_bearing, cp1_magnitude);
                    let (cp2_x, cp2_y) = geometry.coords(cp2_bearing, cp2_magnitude);
                    let (x, y) = geometry.coords(bearing, magnitude);
                    ctx.bezier_curve_to(cp1_x, cp1_y, cp2_x, cp2_y, x, y)
                }
            }
        }
        if self.stroke_style.is_some() {
            ctx.stroke();
        }
    }
}

pub struct Circle<G: Geometry> {
    r: f64,
    coords: G::Coords,
}

impl<G: Geometry> Circle<G> {
    pub fn new(coords: G::Coords, r: f64) -> Self {
        Self { coords, r }
    }
}

impl Drawable<Cartesian> for Circle<Cartesian> {
    fn draw(&self, ctx: &web_sys::CanvasRenderingContext2d, _: &Cartesian) {
        ctx.begin_path();
        let (cx, cy) = self.coords;
        ctx.arc(cx, cy, self.r, 0., 2. * std::f64::consts::PI)
            .unwrap();
        ctx.fill();
    }
}

pub struct Text {
    x: f64,
    y: f64,
    text: String,
}

impl Text {
    pub const fn new(x: f64, y: f64, text: String) -> Self {
        Self { x, y, text }
    }
}

impl Drawable<Cartesian> for Text {
    fn draw(&self, ctx: &web_sys::CanvasRenderingContext2d, _: &Cartesian) {
        ctx.fill_text(&self.text, self.x, self.y).unwrap();
    }
}
