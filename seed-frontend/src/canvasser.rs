use enclose::enclose;
use seed::prelude::*;
use seed::util;
use seed::util::ClosureNew;
use std::cell::{Cell, Ref, RefCell, RefMut};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use wasm_bindgen::closure::Closure;
use web_sys::HtmlCanvasElement;

pub mod animate;
pub mod draw;

use animate::*;

#[derive(Copy, Clone, Debug)]
struct RenderInfo {
    pub timestamp: f64,
    pub timestamp_delta: Option<f64>,
}

pub type ShouldDrawFn<Mdl, TCtx> = fn(&Mdl, u64, bool) -> Option<TCtx>;

/// This gets added into the model of the seed app and referred to by the seed view
/// ```
/// #[derive(Default)]
/// struct Model {
///    canvas: App<DrawModel, TimingContext>,
/// }
/// ```
pub struct App<Mdl: 'static + Animatable<TCtx>, TCtx> {
    /// App configuration available for the entire application lifetime.
    cfg: Rc<AppCfg<Mdl, TCtx>>,
    /// Mutable app state.
    data: Rc<AppData<Mdl, TCtx>>,
}

impl<Mdl: Animatable<TCtx>, TCtx> Clone for App<Mdl, TCtx> {
    fn clone(&self) -> Self {
        Self {
            cfg: Rc::clone(&self.cfg),
            data: Rc::clone(&self.data),
        }
    }
}

impl<Mdl: Animatable<TCtx>, TCtx: 'static> App<Mdl, TCtx>
where
    Mdl::TransitionContext: TransitionContext + Default,
{
    pub fn new(update: ShouldDrawFn<Mdl, TCtx>, model: Mdl) -> App<Mdl, TCtx> {
        App {
            cfg: Rc::new(AppCfg {
                canvas: ElRef::new(),
                update,
            }),
            data: Rc::new(AppData {
                model: RefCell::new(model),
                transition_ctx: RefCell::new(Mdl::TransitionContext::default()),
                scheduled_render_handle: RefCell::new(None),
                render_info: Cell::new(None),
                frame_count: AtomicU64::default(),
            }),
        }
    }

    pub fn model(&self) -> Ref<Mdl> {
        self.data.model.borrow()
    }

    pub fn model_mut(&mut self) -> RefMut<Mdl> {
        self.data.model.borrow_mut()
    }

    pub fn canvas_ref(&self) -> CanvasRef<Mdl, TCtx> {
        CanvasRef(self)
    }

    pub fn is_in_transition(&self) -> bool {
        self.data.transition_ctx.borrow().is_in_transition()
    }

    fn schedule_render(&self) {
        let mut scheduled_render_handle = self.data.scheduled_render_handle.borrow_mut();

        if scheduled_render_handle.is_none() {
            scheduled_render_handle.take();
            let cb = Closure::new(enclose!((self => s) move |_| {
                // remove handle for this timer
                s.data.scheduled_render_handle.borrow_mut().take();
                // stop the render loop if there is no canvas on the page
                if s.cfg.canvas.get().is_some() {
                    s.schedule_render();
                    let frame_count = s.data.frame_count.fetch_add(1, Ordering::SeqCst);
                    let should_draw = (s.cfg.update)(&mut *s.data.model.borrow_mut(), frame_count, s.is_in_transition());

                    if let Some(timing) = should_draw {
                        s.rerender(timing);
                    }
                }
            }));

            *scheduled_render_handle = Some(util::request_animation_frame(cb));
        }
    }

    fn rerender(&self, timing_ctx: TCtx) {
        let new_render_timestamp = util::window()
            .performance()
            .expect("get `Performance`")
            .now();

        let canvas = self.cfg.canvas.get().expect("get canvas element");

        let canvas_ctx = seed::canvas_context_2d(&canvas);
        canvas_ctx
            .set_global_composite_operation("source-over")
            .unwrap();
        canvas_ctx.clear_rect(0., 0., canvas.width().into(), canvas.height().into());
        canvas_ctx.set_transform(2., 0., 0., 2., 0., 0.).unwrap();

        let mut transition_ctx = self.data.transition_ctx.borrow_mut();
        self.data.model.borrow().draw_frame(
            &timing_ctx,
            &mut transition_ctx,
            &canvas_ctx,
            &draw::Cartesian,
        );

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
    }
}

pub struct CanvasRef<'a, Mdl: Animatable<TCtx> + 'static, TCtx>(&'a App<Mdl, TCtx>);

impl<'a, Ms, Mdl: Animatable<TCtx>, TCtx: 'static> UpdateEl<Ms> for CanvasRef<'a, Mdl, TCtx>
where
    Mdl::TransitionContext: TransitionContext + Default,
{
    /// Canvasser is added to the Seed VDom, once the render is finished we'll be connected to a canvas on the page and so should ensure the animation loop is started
    fn update_el(self, el: &mut El<Ms>) {
        self.0.cfg.canvas.clone().update_el(el);
        self.0.schedule_render();
    }
}

struct AppData<Mdl: Animatable<TCtx>, TCtx> {
    model: RefCell<Mdl>,
    render_info: Cell<Option<RenderInfo>>,
    frame_count: AtomicU64,
    transition_ctx: RefCell<Mdl::TransitionContext>,

    scheduled_render_handle: RefCell<Option<util::RequestAnimationFrameHandle>>,
}

pub struct AppCfg<Mdl, TCtx>
where
    Mdl: 'static,
{
    update: ShouldDrawFn<Mdl, TCtx>,

    canvas: ElRef<HtmlCanvasElement>,
}
