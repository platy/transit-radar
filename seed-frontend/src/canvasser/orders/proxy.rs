use super::{
    super::{
        App,
        CmdManager,
        Drawable,
        RenderInfo, // CmdHandle, StreamHandle, StreamManager, SubHandle,
        UndefinedGMsg,
    },
    Orders, OrdersContainer,
};
use futures::future::{Future, FutureExt};
// use futures::stream::{Stream, StreamExt};
use std::{
    // any::{Any, TypeId},
    convert::identity,
    rc::Rc,
};

macro_rules! map_callback_return_to_option_ms {
    ($cb_type:ty, $callback:expr, $panic_text:literal, $output_type:tt) => {{
        let t_type = std::any::TypeId::of::<MsU>();
        if t_type == std::any::TypeId::of::<Ms>() {
            $output_type::new(move |value| {
                (&mut Some($callback(value)) as &mut dyn std::any::Any)
                    .downcast_mut::<Option<Ms>>()
                    .and_then(Option::take)
            })
        } else if t_type == std::any::TypeId::of::<Option<Ms>>() {
            $output_type::new(move |value| {
                (&mut $callback(value) as &mut dyn std::any::Any)
                    .downcast_mut::<Option<Ms>>()
                    .and_then(Option::take)
            })
        } else if t_type == std::any::TypeId::of::<()>() {
            $output_type::new(move |value| {
                $callback(value);
                None
            }) as $output_type<$cb_type>
        } else {
            panic!($panic_text);
        }
    }};
}

#[allow(clippy::module_name_repetitions)]
pub struct OrdersProxy<
    'a,
    Ms,
    AppMs: 'static,
    Mdl: 'static,
    Drwble: Drawable,
    GMs: 'static = UndefinedGMsg,
> {
    orders_container: &'a mut OrdersContainer<AppMs, Mdl, Drwble, GMs>,
    f: Rc<dyn Fn(Ms) -> AppMs>,
}

impl<'a, Ms: 'static, AppMs: 'static, Mdl, Drwble: Drawable, GMs>
    OrdersProxy<'a, Ms, AppMs, Mdl, Drwble, GMs>
{
    pub fn new(
        orders_container: &'a mut OrdersContainer<AppMs, Mdl, Drwble, GMs>,
        f: impl Fn(Ms) -> AppMs + 'static,
    ) -> Self {
        OrdersProxy {
            orders_container,
            f: Rc::new(f),
        }
    }
}

impl<'a, Ms: 'static, AppMs: 'static, Mdl, Drwble: Drawable + 'static, GMs> Orders<Ms, GMs>
    for OrdersProxy<'a, Ms, AppMs, Mdl, Drwble, GMs>
{
    type AppMs = AppMs;
    type Mdl = Mdl;
    type Drwble = Drwble;

    fn proxy<ChildMs: 'static>(
        &mut self,
        f: impl FnOnce(ChildMs) -> Ms + 'static + Clone,
    ) -> OrdersProxy<ChildMs, AppMs, Mdl, Drwble, GMs> {
        let previous_f = self.f.clone();
        OrdersProxy {
            orders_container: self.orders_container,
            f: Rc::new(move |child_ms| previous_f(f.clone()(child_ms))),
        }
    }

    fn render(&mut self) -> &mut Self {
        self.orders_container.render();
        self
    }

    // fn force_render_now(&mut self) -> &mut Self {
    //     self.orders_container.force_render_now();
    //     self
    // }

    fn skip(&mut self) -> &mut Self {
        self.orders_container.skip();
        self
    }

    fn animate(&mut self) -> &mut Self {
        self.orders_container.animate();
        self
    }

    fn dont_animate(&mut self) -> &mut Self {
        self.orders_container.dont_animate();
        self
    }

    // fn notify(&mut self, message: impl Any + Clone) -> &mut Self {
    //     self.orders_container.notify(message);
    //     self
    // }

    #[allow(clippy::redundant_closure)]
    fn send_msg(&mut self, msg: Ms) -> &mut Self {
        let f = self.f.clone();
        self.orders_container.send_msg(f(msg));
        self
    }

    #[allow(clippy::redundant_closure)]
    fn perform_cmd<MsU: 'static>(&mut self, cmd: impl Future<Output = MsU> + 'static) -> &mut Self {
        let f = self.f.clone();
        let app = self.clone_app();

        let handler = map_callback_return_to_option_ms!(
            dyn Fn(MsU) -> Option<Ms>,
            identity,
            "Cmds can return only Msg, Option<Msg> or ()!",
            Box
        );

        let cmd = cmd.map(move |msg| {
            if let Some(msg) = handler(msg) {
                app.update(f(msg))
            }
        });
        CmdManager::perform_cmd(cmd);
        self
    }

    // fn perform_cmd_with_handle<MsU: 'static>(
    //     &mut self,
    //     cmd: impl Future<Output = MsU> + 'static,
    // ) -> CmdHandle {
    //     let f = self.f.clone();
    //     let app = self.clone_app();

    //     let handler = map_callback_return_to_option_ms!(
    //         dyn Fn(MsU) -> Option<Ms>,
    //         identity,
    //         "Cmds can return only Msg, Option<Msg> or ()!",
    //         Box
    //     );

    //     let cmd = cmd.map(move |msg| {
    //         if let Some(msg) = handler(msg) {
    //             app.update(f(msg))
    //         }
    //     });
    //     CmdManager::perform_cmd_with_handle(cmd)
    // }

    fn send_g_msg(&mut self, g_msg: GMs) -> &mut Self {
        self.orders_container.send_g_msg(g_msg);
        self
    }

    fn perform_g_cmd(&mut self, g_cmd: impl Future<Output = GMs> + 'static) -> &mut Self {
        self.orders_container.perform_g_cmd(g_cmd);
        self
    }

    // fn perform_g_cmd_with_handle(
    //     &mut self,
    //     g_cmd: impl Future<Output = GMs> + 'static,
    // ) -> CmdHandle {
    //     self.orders_container.perform_g_cmd_with_handle(g_cmd)
    // }

    fn clone_app(&self) -> App<Self::AppMs, Self::Mdl, Self::Drwble, GMs> {
        self.orders_container.clone_app()
    }

    #[allow(clippy::redundant_closure)]
    fn msg_mapper(&self) -> Box<dyn Fn(Ms) -> Self::AppMs> {
        let f = self.f.clone();
        Box::new(move |ms| f(ms))
    }

    fn after_next_render<MsU: 'static>(
        &mut self,
        callback: impl FnOnce(RenderInfo) -> MsU + 'static,
    ) -> &mut Self {
        let callback = map_callback_return_to_option_ms!(
            dyn FnOnce(RenderInfo) -> Option<Ms>,
            callback,
            "Callback can return only Msg, Option<Msg> or ()!",
            Box
        );

        let f = self.f.clone();
        self.clone_app()
            .data
            .after_next_render_callbacks
            .borrow_mut()
            .push(Box::new(move |render_info| {
                callback(render_info).map(|ms| f(ms))
            }));
        self
    }

    // fn subscribe<MsU: 'static, SubMs: 'static + Clone>(
    //     &mut self,
    //     handler: impl FnOnce(SubMs) -> MsU + Clone + 'static,
    // ) -> &mut Self {
    //     let handler = map_callback_return_to_option_ms!(
    //         dyn Fn(SubMs) -> Option<Ms>,
    //         handler.clone(),
    //         "Handler can return only Msg, Option<Msg> or ()!",
    //         Rc
    //     );

    //     let f = self.f.clone();
    //     self.clone_app()
    //         .data
    //         .sub_manager
    //         .borrow_mut()
    //         .subscribe(move |sub_ms| handler(sub_ms).map(|ms| f(ms)));
    //     self
    // }

    // fn subscribe_with_handle<MsU: 'static, SubMs: 'static + Clone>(
    //     &mut self,
    //     handler: impl FnOnce(SubMs) -> MsU + Clone + 'static,
    // ) -> SubHandle {
    //     let handler = map_callback_return_to_option_ms!(
    //         dyn Fn(SubMs) -> Option<Ms>,
    //         handler.clone(),
    //         "Handler can return only Msg, Option<Msg> or ()!",
    //         Rc
    //     );

    //     let f = self.f.clone();
    //     self.clone_app()
    //         .data
    //         .sub_manager
    //         .borrow_mut()
    //         .subscribe_with_handle(move |sub_ms| handler(sub_ms).map(|ms| f(ms)))
    // }

    // fn stream<MsU: 'static>(&mut self, stream: impl Stream<Item = MsU> + 'static) -> &mut Self {
    //     let f = self.f.clone();
    //     let app = self.clone_app();

    //     let handler = map_callback_return_to_option_ms!(
    //         dyn Fn(MsU) -> Option<Ms>,
    //         identity,
    //         "Streams can stream only Msg, Option<Msg> or ()!",
    //         Box
    //     );

    //     let stream = stream.map(move |msg| {
    //         if let Some(msg) = handler(msg) {
    //             app.update(f(msg))
    //         }
    //     });
    //     StreamManager::stream(stream);
    //     self
    // }

    // fn stream_with_handle<MsU: 'static>(
    //     &mut self,
    //     stream: impl Stream<Item = MsU> + 'static,
    // ) -> StreamHandle {
    //     let f = self.f.clone();
    //     let app = self.clone_app();

    //     let handler = map_callback_return_to_option_ms!(
    //         dyn Fn(MsU) -> Option<Ms>,
    //         identity,
    //         "Streams can stream only Msg, Option<Msg> or ()!",
    //         Box
    //     );

    //     let stream = stream.map(move |msg| {
    //         if let Some(msg) = handler(msg) {
    //             app.update(f(msg))
    //         }
    //     });
    //     StreamManager::stream_with_handle(stream)
    // }
}
