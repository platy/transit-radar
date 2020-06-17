use enclose::enclose;
use seed::prelude::*;
use seed::util;
use seed::util::ClosureNew;
use std::cell::{Cell, Ref, RefCell, RefMut};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use wasm_bindgen::closure::Closure;
use web_sys::HtmlCanvasElement;

#[derive(Copy, Clone, Debug)]
struct RenderInfo {
    pub timestamp: f64,
    pub timestamp_delta: Option<f64>,
}

pub type ShouldDrawFn<Mdl> = fn(&Mdl, u64) -> bool;
pub type DrawFn<Mdl> = fn(&Mdl, &web_sys::CanvasRenderingContext2d);

/// This gets added into the model of the seed app and referred to by the seed view
/// ```
/// #[derive(Default)]
/// struct Model {
///    canvas: App,
/// }
/// ```
pub struct App<Mdl: 'static> {
    /// App configuration available for the entire application lifetime.
    cfg: Rc<AppCfg<Mdl>>,
    /// Mutable app state.
    data: Rc<AppData<Mdl>>,
}

impl<Mdl> Clone for App<Mdl> {
    fn clone(&self) -> Self {
        Self {
            cfg: Rc::clone(&self.cfg),
            data: Rc::clone(&self.data),
        }
    }
}

impl<Mdl> App<Mdl> {
    pub fn new(update: ShouldDrawFn<Mdl>, view: DrawFn<Mdl>, model: Mdl) -> App<Mdl> {
        App {
            cfg: Rc::new(AppCfg {
                canvas: ElRef::new(),
                update,
                view,
            }),
            data: Rc::new(AppData {
                model: RefCell::new(model),
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

    pub fn canvas_ref(&self) -> CanvasRef<Mdl> {
        CanvasRef(self)
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
                    let should_draw = (s.cfg.update)(&mut *s.data.model.borrow_mut(), frame_count);

                    if should_draw {
                        s.rerender();
                    }
                }
            }));

            *scheduled_render_handle = Some(util::request_animation_frame(cb));
        }
    }

    fn rerender(&self) {
        let new_render_timestamp = util::window()
            .performance()
            .expect("get `Performance`")
            .now();

        let canvas = self.cfg.canvas.get().expect("get canvas element");

        let ctx = seed::canvas_context_2d(&canvas);
        ctx.set_global_composite_operation("source-over").unwrap();
        ctx.clear_rect(0., 0., canvas.width().into(), canvas.height().into());
        ctx.set_transform(2., 0., 0., 2., 0., 0.).unwrap();

        (self.cfg.view)(&self.data.model.borrow(), &ctx);

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

pub struct CanvasRef<'a, Mdl: 'static>(&'a App<Mdl>);

impl<'a, Ms, Mdl> UpdateEl<Ms> for CanvasRef<'a, Mdl> {
    /// Canvasser is added to the Seed VDom, once the render is finished we'll be connected to a canvas on the page and so should ensure the animation loop is started
    fn update_el(self, el: &mut El<Ms>) {
        self.0.cfg.canvas.clone().update_el(el);
        self.0.schedule_render();
    }
}

struct AppData<Mdl> {
    model: RefCell<Mdl>,
    render_info: Cell<Option<RenderInfo>>,
    frame_count: AtomicU64,

    scheduled_render_handle: RefCell<Option<util::RequestAnimationFrameHandle>>,
}

pub struct AppCfg<Mdl>
where
    Mdl: 'static,
{
    update: ShouldDrawFn<Mdl>,
    view: DrawFn<Mdl>,

    canvas: ElRef<HtmlCanvasElement>,
}
