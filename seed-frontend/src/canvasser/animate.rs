use super::draw::*;
use std::collections::HashMap;
use web_sys::CanvasRenderingContext2d;

pub trait Animatable<TimingContext, Geometry = Cartesian> {
    type TransitionContext;
    fn draw_frame(
        &self,
        timing_ctx: &TimingContext,
        transition_ctx: &mut Self::TransitionContext,
        canvas: &web_sys::CanvasRenderingContext2d,
        geometry: &Geometry,
    );
}

// // maybe this is just animatables that can be transitioned out?
// pub trait AnimatableCollection<TimingContext, Geometry = Cartesian>: Animatable<TimingContext, Geometry> {
//     type FadeOutCollection: Animatable<TimingContext, Geometry>;
//     fn fade_out(self) -> Self::FadeOutCollection;
// }

impl<T, TimingContext, G> Animatable<TimingContext, G> for Option<T>
where
    T: Animatable<TimingContext, G>,
    T::TransitionContext: Default, // Default context is provided when the element has gone None -> Some
{
    type TransitionContext = Option<T::TransitionContext>;

    fn draw_frame(
        &self,
        timing_ctx: &TimingContext,
        transition_ctx: &mut Self::TransitionContext,
        canvas: &CanvasRenderingContext2d,
        geometry: &G,
    ) {
        if let Some(i) = self {
            let inner_ctx = transition_ctx.get_or_insert_with(Default::default);
            i.draw_frame(timing_ctx, inner_ctx, canvas, geometry);
        } else {
            //@TODO animate fade out
            *transition_ctx = None;
        }
    }
}

impl<T, TimingContext, G> Animatable<TimingContext, G> for Vec<T>
where
    T: Animatable<TimingContext, G>,
    T::TransitionContext: Default, // Default context is provided when the item is added
{
    type TransitionContext = Vec<T::TransitionContext>;

    fn draw_frame(
        &self,
        timing_ctx: &TimingContext,
        transition_ctx: &mut Self::TransitionContext,
        canvas: &CanvasRenderingContext2d,
        geometry: &G,
    ) {
        //@todo fade out removed
        transition_ctx.resize_with(self.len(), Default::default);
        for (ani, ctx) in self.iter().zip(transition_ctx) {
            ani.draw_frame(timing_ctx, ctx, canvas, geometry);
        }
    }
}

impl<K: Copy + Eq + std::hash::Hash, V, TimingContext, G> Animatable<TimingContext, G>
    for HashMap<K, V>
where
    V: Animatable<TimingContext, G>,
    V::TransitionContext: Default, // Default context is provided when the item is added
{
    type TransitionContext = HashMap<K, V::TransitionContext>;

    fn draw_frame(
        &self,
        timing_ctx: &TimingContext,
        transition_ctx: &mut Self::TransitionContext,
        canvas: &CanvasRenderingContext2d,
        geometry: &G,
    ) {
        //@todo fade out removed
        transition_ctx.retain(|k, _v| self.contains_key(k));
        for (k, v) in self {
            let ctx = transition_ctx.entry(*k).or_default();
            v.draw_frame(timing_ctx, ctx, canvas, geometry);
        }
    }
}

// impl<K: Copy + Eq + std::hash::Hash, V, TimingContext, G> AnimatableCollection<TimingContext, G> for HashMap<K, V>
// where
// V: Animatable<TimingContext, G>,
// V::TransitionContext: Default, // Default context is provided when the item is added
// {
//     type FadeOutCollection = FadeOutHashMap<K, V>;

//     /// fades out each of the elements as they are removed from the model
//     fn fade_out(self) -> FadeOutHashMap<K, V> {
//         FadeOutHashMap(self)
//     }
// }

impl Animatable<f64, Polar> for Path<Polar> {
    type TransitionContext = PathTransitionContext;

    fn draw_frame(
        &self,
        _frame_time: &f64,
        _transition_ctx: &mut Self::TransitionContext,
        canvas: &web_sys::CanvasRenderingContext2d,
        geometry: &Polar,
    ) {
        // transition_ctx.process_transition_frame(self, frame_time, 1000.).draw(canvas, geometry)
        self.draw(canvas, geometry)
    }
}

// struct FadeOutHashMap<K, V>(HashMap<K, V>);

// impl<K: Copy + Eq + std::hash::Hash, V, TimingContext, G> Animatable<TimingContext, G> for FadeOutHashMap<K, V>
// where
// V: Animatable<TimingContext, G>,
// V::TransitionContext: Default, // Default context is provided when the item is added
// {
//     type TransitionContext = std::collections::HashMap<K, V::TransitionContext>;

//     fn draw_frame(&self, timing_ctx: &TimingContext, transition_ctx: &mut Self::TransitionContext, canvas: &CanvasRenderingContext2d, geometry: &G) {
//         //@todo fade out removed
//         transition_ctx.retain(|k, _v| self.contains_key(k));
//         for (k, v) in self.0 {
//             let ctx = transition_ctx.entry(*k).or_default();
//             v.draw_frame(timing_ctx, ctx, canvas, geometry);
//         }
//     }
// }

impl<TimingContext, G, D: Drawable<G> + Animatable<TimingContext, G>>
    Animatable<TimingContext, Cartesian> for AsCartesian<G, D>
{
    type TransitionContext = D::TransitionContext;

    fn draw_frame(
        &self,
        timing_ctx: &TimingContext,
        transition_ctx: &mut Self::TransitionContext,
        canvas: &web_sys::CanvasRenderingContext2d,
        _: &Cartesian,
    ) {
        self.shape
            .draw_frame(timing_ctx, transition_ctx, canvas, &self.geometry)
    }
}

pub trait TransitionContext {
    fn is_in_transition(&self) -> bool;
}

impl<T> TransitionContext for Option<T>
where
    T: TransitionContext,
{
    fn is_in_transition(&self) -> bool {
        self.as_ref()
            .map_or(false, TransitionContext::is_in_transition)
    }
}

#[derive(Debug)]
pub enum CartesianTransitionContext {
    /// the element is new
    None,
    /// the element is not moving
    Static { position: (f64, f64) },
    /// the element is in a transition
    Transitioning {
        position: (f64, f64),
        time: f64,
        target: (f64, f64),
        target_time: f64,
    },
}

impl Default for CartesianTransitionContext {
    fn default() -> Self {
        Self::None
    }
}

impl TransitionContext for CartesianTransitionContext {
    fn is_in_transition(&self) -> bool {
        match self {
            Self::Transitioning { .. } => true,
            _ => false,
        }
    }
}

impl CartesianTransitionContext {
    pub fn or_start(&mut self, position: (f64, f64)) -> &mut Self {
        if let Self::None = self {
            *self = Self::Static { position };
        }
        self
    }

    pub fn process_transition_frame(
        &mut self,
        new_target: (f64, f64),
        frame_time: f64,
        transition_duration: f64,
    ) -> (f64, f64) {
        use seed::log;
        match self {
            Self::None => {
                *self = Self::Static {
                    position: new_target,
                };
                new_target
            }
            Self::Static { position } => {
                let (cx, cy) = new_target;
                let (px, py) = *position;
                // how far away is the new target?
                let (dx, dy) = (cx - px, cy - py);
                let sq_distance_to_target = dx.powi(2) + dy.powi(2);
                if sq_distance_to_target > 5. {
                    // start a transition
                    // set velocity and transition clock according to the last position, target, and animation function
                    let velocity = (dx / transition_duration, dy / transition_duration);
                    // calculate position for this frame
                    let elapsed_time = 0.05_f64; // just a random underestimate
                    let position = (
                        px + velocity.0 * elapsed_time,
                        py + velocity.1 * elapsed_time,
                    );
                    *self = Self::Transitioning {
                        position,
                        time: frame_time,
                        target: (cx, cy),
                        target_time: frame_time + transition_duration,
                    };
                    position
                } else {
                    // just draw in the new position, no transition needed
                    *self = Self::Static { position: (cx, cy) };
                    (cx, cy)
                }
            }
            Self::Transitioning {
                position,
                time: previous_time,
                target,
                ref mut target_time,
            } => {
                let (cx, cy) = new_target;
                let (px, py) = *position;
                let (tx, ty) = *target;
                // has the target changed enough to reset the animation timer?
                if (cx - tx).powi(2) + (cy - ty).powi(2) > 5. {
                    // add some time onto the transition clock
                    *target_time = frame_time + transition_duration;
                }
                // time until animation is complete
                let transition_duration_remaining = *target_time as f64 - frame_time;
                // time since last draw
                let elapsed_time = frame_time - *previous_time as f64;
                if transition_duration_remaining > elapsed_time {
                    let (dx, dy) = (cx - px, cy - py);
                    // change velocity according to the last position, target, transition clock and animation function @todo should limit the impulse for each frame
                    let velocity = (
                        dx / transition_duration_remaining,
                        dy / transition_duration_remaining,
                    );
                    // calculate position for this frame
                    let new_position = (
                        px + velocity.0 * elapsed_time,
                        py + velocity.1 * elapsed_time,
                    );
                    *self = Self::Transitioning {
                        position: new_position,
                        time: frame_time,
                        target: (cx, cy),
                        target_time: *target_time,
                    };
                    new_position
                } else {
                    // just draw in the new position, transition is over
                    *self = Self::Static { position: (cx, cy) };
                    new_target
                }
            }
        }
    }
}

pub enum PathTransitionContext {
    /// the path is new
    None,
    /// the path is not moving
    Static { ops: Vec<PathOp<Cartesian>> },
    /// the path is in a transition
    Transitioning {
        ops: Vec<PathOp<TransitioningCartesianGeometry>>,
        time: f64,
        target_time: f64,
    },
}

#[derive(Debug)]
pub struct TransitioningCartesianGeometry;
impl Geometry for TransitioningCartesianGeometry {
    type Coords = TransitionCoords;
}

pub struct TransitionCoords {
    current: (f64, f64),
    velocity: (f64, f64),
    target: (f64, f64),
}

impl Default for PathTransitionContext {
    fn default() -> Self {
        Self::None
    }
}

impl TransitionContext for PathTransitionContext {
    fn is_in_transition(&self) -> bool {
        match self {
            Self::Transitioning { .. } => true,
            _ => false,
        }
    }
}

// impl PathTransitionContext {
//     pub fn or_start(&mut self, position: (f64, f64), new_target: &Vec<PathOp<Cartesian>>) -> &mut Self {
//         match self {
//             Self::None => *self = Self::Static { ops: new_target.iter().map(|path_op| match path_op {
//                 PathOp::MoveTo(_) => PathOp::MoveTo(position),
//                 PathOp::LineTo(_) => PathOp::LineTo(position),
//                 PathOp::BezierCurveTo(_, _, _) => PathOp::BezierCurveTo(position, position, position),
//             }).collect() },
//             _ => (),
//         }
//         self
//     }

//     fn mean_sq_difference(from: Vec<PathOp<Cartesian>>, to: Vec<PathOp<Cartesian>>) -> f64 {

//     }

//     pub fn process_transition_frame(&mut self, new_target: Vec<PathOp<Cartesian>>, frame_time: f64, transition_duration: f64) -> Vec<PathOp<Cartesian>> {
//         match self {
//             Self::None => {
//                 *self = PathTransitionContext::Static { ops: new_target };
//                 new_target
//             }
//             Self::Static { ops } => {
//                 if Self::mean_sq_difference(new_target, ops) > 5. {
//                     // start a transition
//                     let elapsed_time = 0.05f64; // just a random underestimate
//                     // @todo shortening the path without just cutting it off
//                     // change ops to same length as target
//                     let ops = ops.into_iter();
//                     let ops = new_target.map(|target_op| {
//                         let prev_op = ops.next();
//                         match (prev_op, target_op) {
//                             // transition the move
//                             (Some(MoveTo(p)), MoveTo(t)) => ,
//                             // several cases, just move for now
//                             (_, MoveTo(t)) => ,
//                             // transtition the line
//                             (Some(LineTo(p)), LineTo(t)) => ,
//                         }
//                     }).collect();
//                     // set velocity and transition clock according to the last position, target, and animation function
//                     let velocity = (dx / transition_duration, dy / transition_duration);
//                     // calculate position for this frame
//                     let position = (px + velocity.0 * elapsed_time, py + velocity.1 * elapsed_time);
//                     *self = Self::Transitioning { ops, time: frame_time, target_time: frame_time + transition_duration };
//                     position
//                 } else {
//                     // just draw in the new position, no transition needed
//                     *self = Self::Static { ops: new_target };
//                     new_target
//                 }
//             }
//             Self::Transitioning { position, time: previous_time, velocity: _, target, ref mut target_time } => {
//                 let (cx, cy) = new_target;
//                 let (px, py) = *position;
//                 let (tx, ty) = *target;
//                 // has the target changed enough to reset the animation timer?
//                 if (cx-tx)*(cx-tx) + (cy-ty)*(cy-ty) > 5. {
//                     // add some time onto the transition clock
//                     *target_time = frame_time + transition_duration;
//                 }
//                 // time until animation is complete
//                 let transition_duration = *target_time as f64 - frame_time;
//                 // time since last draw
//                 let elapsed_time = frame_time - *previous_time as f64;
//                 if transition_duration > elapsed_time {
//                     let (dx, dy) = (cx-px, cy-py);
//                     // change velocity according to the last position, target, transition clock and animation function @todo should limit the impulse for each frame
//                     let velocity = (dx / transition_duration, dy / transition_duration);
//                     // calculate position for this frame
//                     let position = (px + velocity.0 * elapsed_time, py + velocity.1 * elapsed_time);
//                     *self = Self::Transitioning { position, time: frame_time, velocity, target: (cx, cy), target_time: *target_time };
//                     position
//                 } else {
//                     // just draw in the new position, transition is over
//                     *self = Self::Static { position: (cx, cy) };
//                     new_target
//                 }
//             }
//         }
//     }
// }

// #[cfg(feature = "storybook")]
pub mod storybook {
    use crate::canvasser;
    use crate::canvasser::draw::*;
    use canvasser::animate::*;
    use seed::{prelude::*, *};

    pub fn start() {
        App::start("animate", init, update, view);
    }

    fn init(_: Url, _: &mut impl Orders<Msg>) -> Model {
        Model {
            move_transition: canvasser::App::new(should_draw, MoveTransitionDrawModel::Left),
            appear_path: canvasser::App::new(should_draw, None),
        }
    }

    fn should_draw<Mdl>(_model: &Mdl, frame_count: u64, is_in_transition: bool) -> Option<f64> {
        if is_in_transition || frame_count % 10 == 0 {
            Some(js_sys::Date::now())
        } else {
            None
        }
    }

    struct Model {
        move_transition: canvasser::App<MoveTransitionDrawModel, f64>,
        appear_path: canvasser::App<Option<AsCartesian<Polar, Path<Polar>>>, f64>,
    }

    #[derive(Copy, Clone, Debug)]
    enum MoveTransitionDrawModel {
        Left,
        Right,
    }

    impl Animatable<f64> for MoveTransitionDrawModel {
        type TransitionContext = CartesianTransitionContext;

        fn draw_frame(
            &self,
            &time: &f64,
            transition_ctx: &mut Self::TransitionContext,
            canvas: &web_sys::CanvasRenderingContext2d,
            _: &Cartesian,
        ) {
            let x = match self {
                Self::Left => 50.,
                Self::Right => 500. - 50.,
            };
            let position = transition_ctx.process_transition_frame((x, 30.), time, 1000.);
            Circle::new(position, 20.).draw(canvas, &Cartesian);
        }
    }

    enum Msg {
        MoveTransition,
        ToggleAppearPath,
    }

    fn update(msg: Msg, model: &mut Model, _orders: &mut impl Orders<Msg>) {
        match msg {
            Msg::MoveTransition => {
                let new_model = match *model.move_transition.model() {
                    MoveTransitionDrawModel::Left => MoveTransitionDrawModel::Right,
                    MoveTransitionDrawModel::Right => MoveTransitionDrawModel::Left,
                };
                *model.move_transition.model_mut() = new_model;
            }
            Msg::ToggleAppearPath => {
                let new_model = match model.appear_path.model().as_ref() {
                    None => {
                        let mut appear_path = Path::begin_path();
                        appear_path.move_to((Bearing::degrees(-60.), 5.));
                        appear_path.line_to((Bearing::degrees(-5.), 10.));
                        appear_path.bezier_curve_to(
                            (Bearing::degrees(0.), 15.),
                            (Bearing::degrees(-10.), 20.),
                            (Bearing::degrees(-10.), 20.),
                        );
                        appear_path.set_stroke_style("black");
                        let appear_path_geo = Polar::new(0., 100., (0., 0.), 400.);
                        Some(appear_path.as_cartesian(appear_path_geo))
                    }
                    Some(_m) => None,
                };
                *model.appear_path.model_mut() = new_model;
            }
        }
    }

    fn view(model: &Model) -> Vec<Node<Msg>> {
        nodes![
            div![
                h3!["Move transition"],
                button![
                    format!("{:?}", *model.move_transition.model()),
                    ev(Ev::Click, |_| Msg::MoveTransition),
                ],
                canvas![
                    model.move_transition.canvas_ref(),
                    attrs![
                        At::Width => px(1000),
                        At::Height => px(100),
                    ],
                ],
            ],
            div![
                h3!["Appear path"],
                button![
                    model
                        .appear_path
                        .model()
                        .as_ref()
                        .map_or("Appear", |_| "Disappear"),
                    ev(Ev::Click, |_| Msg::ToggleAppearPath),
                ],
                canvas![
                    model.appear_path.canvas_ref(),
                    attrs![
                        At::Width => px(1000),
                        At::Height => px(100),
                    ],
                ],
            ],
        ]
    }
}
