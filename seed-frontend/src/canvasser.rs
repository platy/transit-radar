//! Canvas animation framework based on seed / elm
//!
//! Key differences:
//! * if animation is switched on, the view is recalled on the frame loop and is not triggered by messages
//! * output is not dom changes but things being drawn on a canvas
//! * timing inputs will be provided to transitionable nodes to ease the animation of transitions
//! * transition frames can be rendered without calling view if the canvas nodes support transitions
//! * in animation mode, view function can be triggered by clock changes of different precisions, or fast as the browser allows

use enclose::enclose;
use seed::prelude::ElRef;
use seed::util;
use seed::util::ClosureNew;
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::iter::FromIterator;
use std::rc::Rc;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsValue;
use web_sys::CanvasRenderingContext2d;
use web_sys::HtmlCanvasElement;

mod cmd_manager;
mod effects;
mod mailbox;
mod message_mapper;
mod orders;
mod render_info;
mod scheduler;

pub use cmd_manager::CmdManager;
use effects::Effect;
use mailbox::Mailbox;
pub use message_mapper::MessageMapper;
pub use orders::*;
pub use render_info::RenderInfo;

pub type UpdateFn<Ms, Mdl, Drwble, GMs> =
    fn(Ms, &mut Mdl, &mut OrdersContainer<Ms, Mdl, Drwble, GMs>);
pub type SinkFn<Ms, Mdl, Drwble, GMs> =
    fn(GMs, &mut Mdl, &mut OrdersContainer<Ms, Mdl, Drwble, GMs>);
pub type ViewFn<Mdl, Drwble> = fn(&Mdl) -> Drwble;

pub struct UndefinedGMsg;

/// This gets added into the model of the seed app and referred to by the seed view
/// ```
/// #[derive(Default)]
/// struct Model {
///    canvas: App,
/// }
/// ```
pub struct App<Ms: 'static, Mdl: 'static, Drwble: Drawable, GMs = UndefinedGMsg> {
    /// App configuration available for the entire application lifetime.
    cfg: Rc<AppCfg<Ms, Mdl, Drwble, GMs>>,
    /// Mutable app state.
    data: Rc<AppData<Ms, Mdl>>,
}

impl<Ms, Mdl, Drwble: Drawable, GMs> Clone for App<Ms, Mdl, Drwble, GMs> {
    fn clone(&self) -> Self {
        Self {
            cfg: Rc::clone(&self.cfg),
            data: Rc::clone(&self.data),
        }
    }
}

/// Used to create and store initial app configuration, ie items passed by the app creator.
pub struct Builder<Ms: 'static, Mdl: 'static, Drwble: Drawable, GMs> {
    update: UpdateFn<Ms, Mdl, Drwble, GMs>,
    sink: Option<SinkFn<Ms, Mdl, Drwble, GMs>>,
    view: ViewFn<Mdl, Drwble>,

    canvas_added: Option<fn() -> Ms>,
}

impl<Ms, Mdl: Default, D: Drawable, GMs> Builder<Ms, Mdl, D, GMs> {
    pub fn canvas_added(mut self, f: fn() -> Ms) -> Self {
        self.canvas_added = Some(f);
        self
    }

    pub fn build(self) -> App<Ms, Mdl, D, GMs> {
        App {
            cfg: Rc::new(AppCfg {
                canvas: ElRef::new(),
                update: self.update,
                view: self.view,
                sink: self.sink,
                canvas_added: self.canvas_added,
            }),
            data: Rc::new(AppData {
                model: RefCell::new(Mdl::default()),
                scheduled_render_handle: RefCell::new(None),
                scheduler: RefCell::new(scheduler::Scheduler::new()),
                after_next_render_callbacks: RefCell::new(Vec::new()),
                render_info: Cell::new(None),
                animate: RefCell::new(false),
            }),
        }
    }
}

impl<Ms, Mdl, Drwble: Drawable + 'static, GMs: 'static> App<Ms, Mdl, Drwble, GMs> {
    pub fn builder(
        update: UpdateFn<Ms, Mdl, Drwble, GMs>,
        view: ViewFn<Mdl, Drwble>,
    ) -> Builder<Ms, Mdl, Drwble, GMs> {
        Builder {
            update,
            sink: None,
            view,

            canvas_added: None,
        }
    }

    /// Reference to be given to the canvas element in the seed view
    /// ```
    /// fn view(model: &Model) -> Node<Msg> {
    ///     canvas![
    ///         &model.canvas.el_ref(),
    ///         attrs![
    ///             At::Width => px(200),
    ///             At::Height => px(200),
    ///         ],
    ///     ]
    /// }
    /// ```
    pub fn el_ref(&self) -> ElRef<HtmlCanvasElement> {
        self.cfg.canvas.clone()
    }

    /// seed has rerendered the vdom - this may mean the canvas has been readded or removed and this needs to be checked
    /// ```
    /// enum Msg {
    ///     /// When a user changes a control
    ///     ControlsMsg(controls::Msg),
    ///     /// After each render is completed
    ///     Rendered,
    /// }
    /// fn update(msg: Msg, model: &mut Model, orders: &mut impl Orders<Msg>) {
    ///     match msg {
    ///         Msg::Rendered => {
    ///             model.canvasser.rendered();
    ///             orders.after_next_render(|_| Msg::Rendered).skip();
    ///         }
    ///     }
    /// }
    /// ```
    pub fn rendered(&mut self) {
        if self.cfg.canvas.get().is_some() {
            // canvas is present on page
            if let Some(canvas_added) = self.cfg.canvas_added {
                self.update(canvas_added());
            }
        }
    }

    /// calls the application's update function with this message and then any resulting messages,
    /// then renders the view if necessary and not in an animation loop
    pub fn update(&self, msg: Ms) {
        let mut queue: VecDeque<Effect<Ms, GMs>> = VecDeque::new();
        queue.push_front(Effect::Msg(msg));
        self.process_effect_queue(queue);
    }

    pub fn sink(&self, g_msg: GMs) {
        let mut queue: VecDeque<Effect<Ms, GMs>> = VecDeque::new();
        queue.push_front(Effect::GMsg(g_msg));
        self.process_effect_queue(queue);
    }

    fn schedule_msg(&mut self, timestamp: u64, msg: Ms) -> &mut Self {
        let f = enclose!((self => s) move || s.update(msg));
        self.data.scheduler.borrow_mut().schedule(timestamp, f);
        self
    }

    fn process_effect_queue(&self, mut queue: VecDeque<Effect<Ms, GMs>>) {
        while let Some(effect) = queue.pop_front() {
            match effect {
                Effect::Msg(msg) => {
                    let mut new_effects = self.process_queue_message(msg);
                    queue.append(&mut new_effects);
                }
                Effect::GMsg(g_msg) => {
                    let mut new_effects = self.process_queue_global_message(g_msg);
                    queue.append(&mut new_effects);
                } // Effect::Notification(notification) => {
                  //     let mut new_effects = self.process_queue_notification(&notification);
                  //     queue.append(&mut new_effects);
                  // }
            }
        }
    }

    fn process_queue_message(&self, message: Ms) -> VecDeque<Effect<Ms, GMs>> {
        // for l in self.data.msg_listeners.borrow().iter() {
        //     (l)(&message)
        // }

        let mut orders = OrdersContainer::new(self.clone());
        (self.cfg.update)(message, &mut *self.data.model.borrow_mut(), &mut orders);

        // self.patch_window_event_handlers();

        if !*self.data.animate.borrow() && orders.should_render {
            self.schedule_render();
        }
        orders.effects
    }

    fn process_queue_global_message(&self, g_message: GMs) -> VecDeque<Effect<Ms, GMs>> {
        let mut orders = OrdersContainer::new(self.clone());

        if let Some(sink) = self.cfg.sink {
            sink(g_message, &mut *self.data.model.borrow_mut(), &mut orders);
        }

        // self.patch_window_event_handlers();

        if orders.should_render {
            self.schedule_render();
        }
        orders.effects
    }

    fn schedule_render(&self) {
        let mut scheduled_render_handle = self.data.scheduled_render_handle.borrow_mut();

        if scheduled_render_handle.is_none() {
            let cb = Closure::new(enclose!((self => s) move |_| {
                s.data.scheduled_render_handle.borrow_mut().take();
                s.rerender();
            }));

            *scheduled_render_handle = Some(util::request_animation_frame(cb));
        }
    }

    pub fn animate(&self) {
        self.data.animate.replace(true);

        self.request_animation_frame();
    }

    pub fn dont_animate(&self) {
        self.data.animate.replace(false);

        self.data.scheduled_render_handle.borrow_mut().take();
    }

    fn request_animation_frame(&self) {
        let cb = Closure::new(enclose!((self => s) move |_| {
            s.rerender();

            s.request_animation_frame();
        }));

        let handle = util::request_animation_frame(cb);
        self.data.scheduled_render_handle.replace(Some(handle));
    }

    fn rerender(&self) {
        let new_render_timestamp = util::window()
            .performance()
            .expect("get `Performance`")
            .now();

        let canvas = self.cfg.canvas.get().expect("get canvas element");
        // get canvas height / width ratio
        let rect = canvas.get_bounding_client_rect();

        let height = rect.height();
        let width = rect.width();

        canvas.set_width(width as u32);
        canvas.set_height(height as u32);

        let ctx = seed::canvas_context_2d(&canvas);
        ctx.set_global_composite_operation("source-over").unwrap();
        ctx.clear_rect(0., 0., width, height);

        // if let Some(radar) = &model.radar {
        //     radar.geometry.start_time = time;
        //     radar.geometry.draw(&ctx);

        //     let data = model.sync.get().unwrap();

        let new = (self.cfg.view)(&self.data.model.borrow());
        new.draw(&ctx);
        // }

        // // Create a new vdom: The top element, and all its children. Does not yet
        // // have associated web_sys elements.
        // let mut new = El::empty(Tag::Placeholder);
        // new.children = (self.cfg.view)(self.data.model.borrow().as_ref().unwrap()).into_nodes();

        // let old = self
        //     .data
        //     .main_el_vdom
        //     .borrow_mut()
        //     .take()
        //     .expect("missing main_el_vdom");

        // patch::patch_els(
        //     &self.cfg.document,
        //     &self.mailbox(),
        //     &self.clone(),
        //     &self.cfg.mount_point,
        //     old.children.into_iter(),
        //     new.children.iter_mut(),
        // );

        // // Now that we've re-rendered, replace our stored El with the new one;
        // // it will be used as the old El next time.
        // self.data.main_el_vdom.borrow_mut().replace(new);

        // // Execute `after_next_render_callbacks`.

        let render_info = match self.data.render_info.take() {
            Some(old_render_info) => RenderInfo {
                timestamp: new_render_timestamp,
                timestamp_delta: Some(new_render_timestamp - old_render_info.timestamp),
            },
            None => RenderInfo {
                timestamp: new_render_timestamp,
                timestamp_delta: None,
            },
        };
        self.data.render_info.set(Some(render_info));

        self.process_effect_queue(
            self.data
                .after_next_render_callbacks
                .replace(Vec::new())
                .into_iter()
                .filter_map(|callback| callback(render_info).map(Effect::Msg))
                .collect(),
        );
    }

    pub fn mailbox(&self) -> Mailbox<Ms> {
        Mailbox::new(enclose!((self => s) move |option_message| {
            if let Some(message) = option_message {
                s.update(message);
            } else {
                s.rerender();
            }
        }))
    }
}

struct AppData<Ms: 'static, Mdl> {
    model: RefCell<Mdl>,
    after_next_render_callbacks: RefCell<Vec<Box<dyn FnOnce(RenderInfo) -> Option<Ms>>>>,
    render_info: Cell<Option<RenderInfo>>,
    scheduler: RefCell<scheduler::Scheduler>,

    // @TODO these 2 should be a single structure
    scheduled_render_handle: RefCell<Option<util::RequestAnimationFrameHandle>>,
    animate: RefCell<bool>,
}

pub struct AppCfg<Ms, Mdl, Drwble, GMs>
where
    Ms: 'static,
    Mdl: 'static,
    Drwble: Drawable,
{
    update: UpdateFn<Ms, Mdl, Drwble, GMs>,
    view: ViewFn<Mdl, Drwble>,
    sink: Option<SinkFn<Ms, Mdl, Drwble, GMs>>,

    canvas: ElRef<HtmlCanvasElement>,

    /// the update fn will be called with this each time that the canvas is added to the page
    canvas_added: Option<fn() -> Ms>,
}

pub trait Drawable {
    fn draw(&self, ctx: &CanvasRenderingContext2d);
}

impl<T> Drawable for Vec<T>
where
    T: Drawable,
{
    fn draw(&self, ctx: &CanvasRenderingContext2d) {
        for i in self {
            i.draw(ctx);
        }
    }
}

impl<T: ?Sized> Drawable for Box<T>
where
    T: Drawable,
{
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
            ops: vec![],
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

    pub fn bezier_curve_to(&mut self, cp1x: f64, cp1y: f64, cp2x: f64, cp2y: f64, x: f64, y: f64) {
        self.ops
            .push(PathOp::BezierCurveTo(cp1x, cp1y, cp2x, cp2y, x, y))
    }
}

impl Drawable for Path {
    fn draw(&self, ctx: &CanvasRenderingContext2d) {
        ctx.begin_path();
        ctx.set_line_width(self.line_width);
        ctx.set_line_dash(&js_sys::Array::from_iter(
            self.line_dash.iter().cloned().map(JsValue::from_f64),
        ))
        .unwrap();
        if let Some(stroke_style) = &self.stroke_style {
            ctx.set_stroke_style(&JsValue::from_str(&stroke_style));
        }

        for op in &self.ops {
            match op {
                &PathOp::MoveTo(x, y) => ctx.move_to(x, y),
                &PathOp::LineTo(x, y) => ctx.line_to(x, y),
                &PathOp::BezierCurveTo(cp1x, cp1y, cp2x, cp2y, x, y) => {
                    ctx.bezier_curve_to(cp1x, cp1y, cp2x, cp2y, x, y)
                }
            }
        }
        if self.stroke_style.is_some() {
            ctx.stroke();
        }
    }
}
