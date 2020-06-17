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
use std::rc::Rc;
use wasm_bindgen::closure::Closure;
use web_sys::HtmlCanvasElement;

pub mod draw;
mod render_info;

pub use draw::Drawable;
pub use render_info::RenderInfo;

pub type UpdateFn<Mdl> = fn(&mut Mdl) -> bool;
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

/// Used to create and store initial app configuration, ie items passed by the app creator.
pub struct Builder<Mdl: 'static> {
    update: UpdateFn<Mdl>,
    view: DrawFn<Mdl>,
}

impl<Mdl> Builder<Mdl> {
    pub fn build(self, model: Mdl) -> App<Mdl> {
        App {
            cfg: Rc::new(AppCfg {
                canvas: ElRef::new(),
                update: self.update,
                view: self.view,
            }),
            data: Rc::new(AppData {
                model: RefCell::new(model),
                scheduled_render_handle: RefCell::new(AnimationFrameHandle::None),
                render_info: Cell::new(None),
            }),
        }
    }
}

impl<Mdl> App<Mdl> {
    pub fn builder(update: UpdateFn<Mdl>, view: DrawFn<Mdl>) -> Builder<Mdl> {
        Builder {
            update,
            view,
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
            self.schedule_render()
        }
    }

    fn schedule_render(&self) {
        let mut scheduled_render_handle = self.data.scheduled_render_handle.borrow_mut();

        if scheduled_render_handle.is_not_render() {
            scheduled_render_handle.take();
            let cb = Closure::new(enclose!((self => s) move |_| {
                s.data.scheduled_render_handle.borrow_mut().take();
                s.schedule_render();
                let should_draw = (s.cfg.update)(&mut *s.data.model.borrow_mut());

                if should_draw {
                    s.rerender();
                }
            }));

            *scheduled_render_handle =
                AnimationFrameHandle::Render(util::request_animation_frame(cb));
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

        // if let Some(radar) = &model.radar {
        //     radar.geometry.start_time = time;
        //     radar.geometry.draw(&ctx);

        //     let data = model.sync.get().unwrap();

        (self.cfg.view)(&self.data.model.borrow(), &ctx);
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
    }
}

struct AppData<Mdl> {
    model: RefCell<Mdl>,
    render_info: Cell<Option<RenderInfo>>,

    scheduled_render_handle: RefCell<AnimationFrameHandle>,
}

enum AnimationFrameHandle {
    None,
    NoRender(util::RequestAnimationFrameHandle),
    Render(util::RequestAnimationFrameHandle),
}

impl AnimationFrameHandle {
    fn is_none(&self) -> bool {
        match self {
            Self::None => true,
            _ => false,
        }
    }

    fn is_not_render(&self) -> bool {
        match self {
            Self::Render(_) => false,
            _ => true,
        }
    }

    fn take(&mut self) -> Self {
        std::mem::take(self)
    }
}

impl Default for AnimationFrameHandle {
    fn default() -> Self {
        AnimationFrameHandle::None
    }
}

pub struct AppCfg<Mdl>
where
    Mdl: 'static,
{
    update: UpdateFn<Mdl>,
    view: DrawFn<Mdl>,

    canvas: ElRef<HtmlCanvasElement>,
}
