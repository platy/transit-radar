use super::{App, Drawable, RenderInfo, UndefinedGMsg}; // CmdHandle, StreamHandle, SubHandle,
                                                       // use futures::stream::Stream;
use std::future::Future;

// @TODO: Add links to doc comment once https://github.com/rust-lang/rust/issues/43466 is resolved
// or use nightly rustdoc. Applicable to the entire code base.

pub mod container;
pub mod proxy;

pub use container::OrdersContainer;
pub use proxy::OrdersProxy;

pub trait Orders<Ms: 'static, GMs = UndefinedGMsg> {
    type AppMs: 'static;
    type Mdl: 'static;
    type Drwble: Drawable + 'static;

    /// Automatically map message type. It allows you to pass `Orders` into child module.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    ///Msg::Child(child_msg) => {
    ///    child::update(child_msg, &mut model.child, &mut orders.proxy(Msg::Child));
    ///}
    /// ```
    fn proxy<ChildMs: 'static>(
        &mut self,
        f: impl FnOnce(ChildMs) -> Ms + 'static + Clone,
    ) -> OrdersProxy<ChildMs, Self::AppMs, Self::Mdl, Self::Drwble, GMs>;

    /// Schedule web page rerender after model update. It's the default behaviour.
    /// No effect if animation is enabled
    fn render(&mut self) -> &mut Self;

    // /// Force web page to rerender immediately after model update.
    // fn force_render_now(&mut self) -> &mut Self;

    /// Don't rerender web page after model update.
    /// No effect if animation is enabled
    fn skip(&mut self) -> &mut Self;

    // /// Notify all subscription handlers that listen for messages with the `message`'s type.
    // ///
    // /// _Note:_ Seed's native subscriptions / `messages` can be also sent - e.g.
    // /// `orders.notify(subs::UrlRequested::new(url))`.
    // /// The most is ignored by the Seed's runtime, but some of them are processed and
    // /// trigger side-effects - e.g. simulate `<a>` link click by sending `subs::UrlRequested`.
    // ///
    // /// # Example
    // ///
    // /// ```rust,no_run
    // ///orders.notify(counter::DoReset);
    // ///orders.notify("Hello!");
    // /// ...
    // ///orders.subscribe(Msg::Reset);  // `Msg::Reset(counter::DoReset)`
    // ///orders.subscribe(|greeting: &'static str| log!(greeting));
    // /// ```
    // ///
    // /// _Note:_: All notifications are pushed to the queue - i.e. `update` function is NOT called immediately.
    // fn notify(&mut self, message: impl Any + Clone) -> &mut Self;

    /// Invoke function `update` with the given `msg`.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    ///orders.msg(Msg::Increment);
    /// ```
    ///
    /// _Note:_: All `msg`s are pushed to the queue - i.e. `update` function is NOT called immediately.
    fn send_msg(&mut self, msg: Ms) -> &mut Self;

    fn schedule_msg(&mut self, timestamp: u64, msg: Ms) -> &mut Self;

    /// Execute `cmd` and send its output (if it's `Msg`) to `update` function.
    ///
    /// Output has to be `Msg`, `Option<Msg>` or `()`.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    ///orders.perform_cmd(cmds::timeout(2000, || Msg::OnTimeout));
    ///orders.perform_cmd(async { log!("Hello!") });
    /// ```
    ///
    /// _Note:_: Use the alternative `perform_cmd_with_handle` to control `cmd`'s lifetime.
    ///
    /// # Panics
    ///
    /// Panics when the output isn't `Msg`, `Option<Msg>` or `()`.
    /// (It will be changed to a compile-time error).
    #[allow(clippy::shadow_unrelated)]
    // @TODO remove `'static`s once `optin_builtin_traits`, `negative_impls`
    // @TODO or https://github.com/rust-lang/rust/issues/41875 is stable
    fn perform_cmd<MsU: 'static>(&mut self, cmd: impl Future<Output = MsU> + 'static) -> &mut Self;

    // /// Execute given `cmd` and send its output (if it's `Msg`) to `update` function.
    // /// - Returns `CmdHandle` that you should save to your `Model`.
    // ///   The `cmd` is aborted on the handle drop.
    // ///
    // /// Output has to be `Msg`, `Option<Msg>` or `()`.
    // ///
    // /// # Example
    // ///
    // /// ```rust,no_run
    // ///let timeout_handle = orders.perform_cmd_with_handle(cmds::timeout(2000, || Msg::OnTimeout));
    // ///let cmd_handle = orders.perform_cmd_with_handle(async { log!("Hello!") });
    // /// ```
    // ///
    // /// # Panics
    // ///
    // /// Panics when the output isn't `Msg`, `Option<Msg>` or `()`.
    // /// (It will be changed to a compile-time error).
    // #[must_use = "cmd is aborted on its handle drop"]
    // #[allow(clippy::shadow_unrelated)]
    // // @TODO remove `'static`s once `optin_builtin_traits`, `negative_impls`
    // // @TODO or https://github.com/rust-lang/rust/issues/41875 is stable
    // fn perform_cmd_with_handle<MsU: 'static>(
    //     &mut self,
    //     cmd: impl Future<Output = MsU> + 'static,
    // ) -> CmdHandle;

    /// Similar to `send_msg`, but calls function `sink` with the given global message.
    fn send_g_msg(&mut self, g_msg: GMs) -> &mut Self;

    /// Similar to `perform_cmd`, but result is send to function `sink`.
    fn perform_g_cmd(&mut self, g_cmd: impl Future<Output = GMs> + 'static) -> &mut Self;

    // /// Similar to `perform_g_cmd`, but result is send to function `sink`.
    // /// - Returns `CmdHandle` that you should save to your `Model`.
    // ///   `cmd` is aborted on the handle drop.
    // ///
    // #[must_use = "cmd is aborted on its handle drop"]
    // fn perform_g_cmd_with_handle(
    //     &mut self,
    //     g_cmd: impl Future<Output = GMs> + 'static,
    // ) -> CmdHandle;

    /// Get app instance. Cloning is cheap because `App` contains only `Rc` fields.
    fn clone_app(&self) -> App<Self::AppMs, Self::Mdl, Self::Drwble, GMs>;

    /// Get the function that maps module's `Msg` to app's (root's) one.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    ///let (app, msg_mapper) = (orders.clone_app(), orders.msg_mapper());
    ///app.update(msg_mapper(Msg::AMessage));
    /// ```
    fn msg_mapper(&self) -> Box<dyn Fn(Ms) -> Self::AppMs>;

    /// Register the callback that will be executed after the next render.
    ///
    /// Callback's only parameter is `RenderInfo` - it has fields `timestamp`
    /// and `timestamp_delta`.
    /// `timestamp_delta` is the difference between the old render timestamp and the new one
    /// and it has value `None` if it's the first rendering.
    ///
    /// - It's useful when you want to use DOM API or make animations.
    /// - You can call this function multiple times - callbacks will be executed in the same order.
    /// - Callback has to return `Msg`, `Option<Msg>` or `()`.
    ///
    /// _Note:_ [performance.now()](https://developer.mozilla.org/en-US/docs/Web/API/Performance/now)
    ///  is used under the hood to get timestamps.
    ///
    /// # Panics
    ///
    /// Panics when the handler doesn't return `Msg`, `Option<Msg>` or `()`.
    /// (It will be changed to a compile-time error).
    #[allow(clippy::shadow_unrelated)]
    // @TODO remove `'static`s once `optin_builtin_traits`, `negative_impls`
    // @TODO or https://github.com/rust-lang/rust/issues/41875 is stable
    fn after_next_render<MsU: 'static>(
        &mut self,
        callback: impl FnOnce(RenderInfo) -> MsU + 'static,
    ) -> &mut Self;

    /// Same idea as `after_next_render` except it will run on the next animation frame even if we don't performma a render
    fn next_frame_end<MsU: 'static>(
        &mut self,
        callback: impl FnOnce(Option<RenderInfo>) -> MsU + 'static,
    ) -> &mut Self;

    // /// Subscribe for messages with the `handler`s input type.
    // ///
    // /// Handler has to return `Msg`, `Option<Msg>` or `()`.
    // ///
    // /// # Example
    // ///
    // /// ```rust,no_run
    // ///orders.subscribe(Msg::Reset);  // `Msg::Reset(counter::DoReset)`
    // ///orders.subscribe(|greeting: &'static str| log!(greeting));
    // ///orders.subscribe(Msg::UrlChanged)  // `update(... Msg::UrlChanged(subs::UrlChanged(url)) =>`
    // /// ...
    // ///orders.notify(counter::DoReset);
    // ///orders.notify("Hello!");
    // /// ```
    // ///
    // /// _Note:_: Use the alternative `subscribe_with_handle` to control `sub`'s lifetime.
    // ///
    // /// # Panics
    // ///
    // /// Panics when the handler doesn't return `Msg`, `Option<Msg>` or `()`.
    // /// (It will be changed to a compile-time error).
    // #[allow(clippy::shadow_unrelated)]
    // // @TODO remove `'static`s once `optin_builtin_traits`, `negative_impls`
    // // @TODO or https://github.com/rust-lang/rust/issues/41875 is stable
    // fn subscribe<MsU: 'static, SubMs: 'static + Clone>(
    //     &mut self,
    //     handler: impl FnOnce(SubMs) -> MsU + Clone + 'static,
    // ) -> &mut Self;

    // /// Subscribe for messages with the `handler`s input type.
    // /// - Returns `SubHandle` that you should save to your `Model`.
    // ///   The `sub` is cancelled on the handle drop.
    // ///
    // /// Handler has to return `Msg`, `Option<Msg>` or `()`.
    // ///
    // /// # Example
    // ///
    // /// ```rust,no_run
    // ///let sub_handle = orders.subscribe_with_handle(Msg::Reset);  // `Msg::Reset(counter::DoReset)`
    // ///orders.subscribe_with_handle(|greeting: &'static str| log!(greeting));
    // ///let url_changed_handle = orders.subscribe_with_handle(Msg::UrlChanged)  // `update(... Msg::UrlChanged(subs::UrlChanged(url)) =>`
    // /// ...
    // ///orders.notify(counter::DoReset);
    // ///orders.notify("Hello!");
    // /// ```
    // ///
    // /// # Panics
    // ///
    // /// Panics when the handler doesn't return `Msg`, `Option<Msg>` or `()`.
    // /// (It will be changed to a compile-time error).
    // #[must_use = "subscription is cancelled on its handle drop"]
    // #[allow(clippy::shadow_unrelated)]
    // // @TODO remove `'static`s once `optin_builtin_traits`, `negative_impls`
    // // @TODO or https://github.com/rust-lang/rust/issues/41875 is stable
    // fn subscribe_with_handle<MsU: 'static, SubMs: 'static + Clone>(
    //     &mut self,
    //     handler: impl FnOnce(SubMs) -> MsU + Clone + 'static,
    // ) -> SubHandle;

    // /// Stream `Msg`, `Option<Msg>` or `()`.
    // ///
    // /// # Example
    // ///
    // /// ```rust,no_run
    // ///orders.stream(streams::interval(1000, || Msg::OnTick));
    // ///orders.stream(streams::window_event(Ev::Resize, |_| Msg::OnResize));
    // /// ```
    // ///
    // /// _Note:_: Use the alternative `stream_with_handle` to control `stream`'s lifetime.
    // ///
    // /// # Panics
    // ///
    // /// Panics when the handler doesn't return `Msg`, `Option<Msg>` or `()`.
    // /// (It will be changed to a compile-time error).
    // #[allow(clippy::shadow_unrelated)]
    // // @TODO remove `'static`s once `optin_builtin_traits`, `negative_impls`
    // // @TODO or https://github.com/rust-lang/rust/issues/41875 is stable
    // fn stream<MsU: 'static>(&mut self, stream: impl Stream<Item = MsU> + 'static) -> &mut Self;

    // /// Stream `Msg`, `Option<Msg>` or `()`.
    // /// - Returns `StreamHandle` that you should save to your `Model`.
    // ///   The `stream` is cancelled on the handle drop.
    // ///
    // /// # Example
    // ///
    // /// ```rust,no_run
    // ///let timer_handler = orders.stream_with_handle(streams::interval(1000, || Msg::OnTick));
    // ///let stream_handler = orders.stream_with_handle(streams::window_event(Ev::Resize, |_| Msg::OnResize));
    // /// ```
    // ///
    // /// # Panics
    // ///
    // /// Panics when the handler doesn't return `Msg`, `Option<Msg>` or `()`.
    // /// (It will be changed to a compile-time error).
    // #[must_use = "stream is stopped on its handle drop"]
    // #[allow(clippy::shadow_unrelated)]
    // // @TODO remove `'static`s once `optin_builtin_traits`, `negative_impls`
    // // @TODO or https://github.com/rust-lang/rust/issues/41875 is stable
    // fn stream_with_handle<MsU: 'static>(
    //     &mut self,
    //     stream: impl Stream<Item = MsU> + 'static,
    // ) -> StreamHandle;

    // /// Cheap clone base path loaded from element `<base href="base/path">`.
    // ///
    // /// Returns empty `Vec` if there is no `base` element in your HTML
    // /// or there were problems with parsing.
    // fn clone_base_path(&self) -> Rc<Vec<String>> {
    //     Rc::clone(&self.clone_app().cfg.base_path)
    // }
}
