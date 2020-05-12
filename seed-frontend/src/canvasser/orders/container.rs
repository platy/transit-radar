use super::{
    super::{
        effects::Effect,
        App,
        CmdManager,
        Drawable,
        RenderInfo, // render_timestamp_delta::RenderTimestampDelta, CmdHandle,
                    // Notification, ShouldRender, StreamHandle, StreamManager, SubHandle
        UndefinedGMsg,
    },
    Orders, OrdersProxy,
};
use futures::future::FutureExt;
// use futures::stream::{Stream, StreamExt};
use std::{
    // any::{Any, TypeId},
    collections::VecDeque,
    convert::identity,
    future::Future,
    // rc::Rc,
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
pub struct OrdersContainer<Ms: 'static, Mdl: 'static, Drwble: Drawable, GMs = UndefinedGMsg> {
    pub(crate) should_render: bool,
    pub(crate) effects: VecDeque<Effect<Ms, GMs>>,
    app: App<Ms, Mdl, Drwble, GMs>,
}

impl<Ms, Mdl, Drwble: Drawable, GMs> OrdersContainer<Ms, Mdl, Drwble, GMs> {
    pub fn new(app: App<Ms, Mdl, Drwble, GMs>) -> Self {
        Self {
            should_render: true,
            effects: VecDeque::new(),
            app,
        }
    }

    // pub(crate) fn merge(&mut self, mut other: Self) {
    //     self.should_render = other.should_render;
    //     self.effects.append(&mut other.effects);
    // }
}

impl<Ms: 'static, Mdl, Drwble: Drawable + 'static, GMs: 'static> Orders<Ms, GMs>
    for OrdersContainer<Ms, Mdl, Drwble, GMs>
{
    type AppMs = Ms;
    type Mdl = Mdl;
    type Drwble = Drwble;

    #[allow(clippy::redundant_closure)]
    fn proxy<ChildMs: 'static>(
        &mut self,
        f: impl FnOnce(ChildMs) -> Ms + 'static + Clone,
    ) -> OrdersProxy<ChildMs, Ms, Mdl, Drwble, GMs> {
        OrdersProxy::new(self, move |child_ms| f.clone()(child_ms))
    }

    fn render(&mut self) -> &mut Self {
        self.should_render = true;
        self
    }

    // fn force_render_now(&mut self) -> &mut Self {
    //     self.should_render = ShouldRender::ForceRenderNow;
    //     self
    // }

    fn skip(&mut self) -> &mut Self {
        self.should_render = false;
        self
    }

    fn animate(&mut self) -> &mut Self {
        self.app.animate();
        self
    }

    fn dont_animate(&mut self) -> &mut Self {
        self.app.dont_animate();
        self
    }

    // fn notify(&mut self, message: impl Any + Clone) -> &mut Self {
    //     self.effects
    //         .push_back(Effect::Notification(Notification::new(message)));
    //     self
    // }

    fn send_msg(&mut self, msg: Ms) -> &mut Self {
        self.effects.push_back(msg.into());
        self
    }

    fn perform_cmd<MsU: 'static>(&mut self, cmd: impl Future<Output = MsU> + 'static) -> &mut Self {
        let app = self.app.clone();

        let handler = map_callback_return_to_option_ms!(
            dyn Fn(MsU) -> Option<Ms>,
            identity,
            "Cmds can return only Msg, Option<Msg> or ()!",
            Box
        );

        let cmd = cmd.map(move |msg| app.mailbox().send(handler(msg)));
        CmdManager::perform_cmd(cmd);
        self
    }

    // fn perform_cmd_with_handle<MsU: 'static>(
    //     &mut self,
    //     cmd: impl Future<Output = MsU> + 'static,
    // ) -> CmdHandle {
    //     let app = self.app.clone();

    //     let handler = map_callback_return_to_option_ms!(
    //         dyn Fn(MsU) -> Option<Ms>,
    //         identity,
    //         "Cmds can return only Msg, Option<Msg> or ()!",
    //         Box
    //     );

    //     let cmd = cmd.map(move |msg| {
    //         if let Some(msg) = handler(msg) {
    //             app.update(msg)
    //         }
    //     });
    //     CmdManager::perform_cmd_with_handle(cmd)
    // }

    fn send_g_msg(&mut self, g_msg: GMs) -> &mut Self {
        let effect = Effect::GMsg(g_msg);
        self.effects.push_back(effect);
        self
    }

    fn perform_g_cmd(&mut self, cmd: impl Future<Output = GMs> + 'static) -> &mut Self {
        let app = self.app.clone();
        let cmd = cmd.map(move |msg| app.sink(msg));
        CmdManager::perform_cmd(cmd);
        self
    }

    // fn perform_g_cmd_with_handle(&mut self, cmd: impl Future<Output = GMs> + 'static) -> CmdHandle {
    //     let app = self.app.clone();
    //     let cmd = cmd.map(move |msg| app.sink(msg));
    //     CmdManager::perform_cmd_with_handle(cmd)
    // }

    fn clone_app(&self) -> App<Self::AppMs, Self::Mdl, Self::Drwble, GMs> {
        self.app.clone()
    }

    fn msg_mapper(&self) -> Box<dyn Fn(Ms) -> Self::AppMs> {
        Box::new(identity)
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

        self.app
            .data
            .after_next_render_callbacks
            .borrow_mut()
            .push(callback);
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

    //     self.app
    //         .data
    //         .sub_manager
    //         .borrow_mut()
    //         .subscribe(move |sub_ms| handler(sub_ms));
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

    //     self.app
    //         .data
    //         .sub_manager
    //         .borrow_mut()
    //         .subscribe_with_handle(move |sub_ms| handler(sub_ms))
    // }

    // fn stream<MsU: 'static>(&mut self, stream: impl Stream<Item = MsU> + 'static) -> &mut Self {
    //     let app = self.app.clone();

    //     let handler = map_callback_return_to_option_ms!(
    //         dyn Fn(MsU) -> Option<Ms>,
    //         identity,
    //         "Streams can stream only Msg, Option<Msg> or ()!",
    //         Box
    //     );

    //     let stream = stream.map(move |msg| {
    //         if let Some(msg) = handler(msg) {
    //             app.update(msg)
    //         }
    //     });
    //     StreamManager::stream(stream);
    //     self
    // }

    // fn stream_with_handle<MsU: 'static>(
    //     &mut self,
    //     stream: impl Stream<Item = MsU> + 'static,
    // ) -> StreamHandle {
    //     let app = self.app.clone();

    //     let handler = map_callback_return_to_option_ms!(
    //         dyn Fn(MsU) -> Option<Ms>,
    //         identity,
    //         "Streams can stream only Msg, Option<Msg> or ()!",
    //         Box
    //     );

    //     let stream = stream.map(move |msg| {
    //         if let Some(msg) = handler(msg) {
    //             app.update(msg)
    //         }
    //     });
    //     StreamManager::stream_with_handle(stream)
    // }
}
