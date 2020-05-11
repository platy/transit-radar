//! Canvas animation framework based on seed / elm
//! 
//! Key differences:
//! * if animation is switched on, the view is recalled on the frame loop and is not triggered by messages
//! * output is not dom changes but things being drawn on a canvas

use std::iter::FromIterator;
use web_sys::CanvasRenderingContext2d;
use wasm_bindgen::JsValue;


pub trait Drawable {
  fn draw(&self, ctx: &CanvasRenderingContext2d);
}

impl<T> Drawable for Vec<T>
where T: Drawable {
    fn draw(&self, ctx: &CanvasRenderingContext2d) {
        for i in self {
            i.draw(ctx);
        }
    }
}

impl<T> Drawable for Box<T>
where T: Drawable {
    fn draw(&self, ctx: &CanvasRenderingContext2d) {
        self.as_ref().draw(ctx);
    }
}

pub struct Path {
  line_width: f64,
  line_dash: Vec<f64>,
  stroke_style: Option<String>,
  ops: Vec<PathOp>,
}

enum PathOp {
  MoveTo(f64, f64),
  LineTo(f64, f64),
  BezierCurveTo(f64, f64, f64, f64, f64, f64),
}

impl Path {
  pub fn begin_path() -> Path {
      Path {
          line_width: 1.,
          line_dash: vec![],
          stroke_style: None,
          ops: vec!(),
      }
  }

  pub fn set_line_width(&mut self, line_width: f64) {
      self.line_width = line_width;
  }

  pub fn set_line_dash(&mut self, line_dash: &[f64]) {
      self.line_dash = line_dash.into();
  }

  pub fn set_stroke_style(&mut self, stroke_style: &str) {
      self.stroke_style = Some(stroke_style.to_owned());
  }

  pub fn move_to(&mut self, x: f64, y: f64) {
      self.ops.push(PathOp::MoveTo(x, y));
  }

  pub fn line_to(&mut self, x: f64, y: f64) {
      self.ops.push(PathOp::LineTo(x, y));
  }

  pub fn bezier_curve_to(&mut self,  cp1x: f64, cp1y: f64, cp2x: f64, cp2y: f64, x: f64, y: f64) {
      self.ops.push(PathOp::BezierCurveTo(cp1x, cp1y, cp2x, cp2y, x, y))
  }
}

impl Drawable for Path {
  fn draw(&self, ctx: &CanvasRenderingContext2d) {
      ctx.begin_path();
      ctx.set_line_width(self.line_width);
      ctx.set_line_dash(&js_sys::Array::from_iter(self.line_dash.iter().cloned().map(JsValue::from_f64))).unwrap();
      if let Some(stroke_style) = &self.stroke_style {
          ctx.set_stroke_style(&JsValue::from_str(&stroke_style));
      }

      for op in &self.ops {
          match op {
              &PathOp::MoveTo(x, y) => ctx.move_to(x, y),
              &PathOp::LineTo(x, y) => ctx.line_to(x, y),
              &PathOp::BezierCurveTo(cp1x, cp1y, cp2x, cp2y, x, y) => ctx.bezier_curve_to(cp1x, cp1y, cp2x, cp2y, x, y),
          }
      }
      if self.stroke_style.is_some() {
          ctx.stroke();
      }
  }
}
