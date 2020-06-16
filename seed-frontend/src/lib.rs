use seed::{prelude::*, *};

mod canvasser;
mod controls;
mod radar;
mod sync;

#[wasm_bindgen(start)]
pub fn render() {
    App::start("app", init, update, view);
}

fn init(url: Url, orders: &mut impl Orders<Msg>) -> Model {
    orders.after_next_render(|_| Msg::Rendered);

    Model {
        canvasser: radar::init(),
        controls: controls::Model::init(url, &mut orders.proxy(Msg::ControlsMsg)),
    }
}

/// fn after_mount(_: Url, orders: &mut impl Orders<Msg>) -> AfterMount<Model> {
///     orders.after_next_render(|_| Msg::Rendered);
/// // ...
///
/// fn update(msg: Msg, model: &mut Model, orders: &mut impl Orders<Msg>) {
///     match msg {
///         Msg::Rendered => {
///             let canvas = canvas.get().expect("get canvas element");
///             // ...
///             orders.after_next_render(|_| Msg::Rendered).skip();
///         }

struct Model {
    canvasser: canvasser::App<radar::CanvasMsg, radar::CanvasModel>,
    controls: controls::Model,
}

enum Msg {
    /// When a user changes a control
    ControlsMsg(controls::Msg),
    /// After each render is completed
    Rendered,
}

fn update(msg: Msg, model: &mut Model, orders: &mut impl Orders<Msg>) {
    match msg {
        Msg::ControlsMsg(msg) => {
            if controls::update(
                msg,
                &mut model.controls,
                &mut orders.proxy(Msg::ControlsMsg),
            ) {
                model.canvasser.update(radar::CanvasMsg::ControlsChange(
                    model.controls.params.clone(),
                ));
            }
        }
        Msg::Rendered => {
            model.canvasser.rendered();
            orders.after_next_render(|_| Msg::Rendered).skip();
        }
    }
}

fn view(model: &Model) -> Node<Msg> {
    // if let Some(data) = model.sync.get() {
    div![
        h2![&model
            .controls
            .params
            .station_selection
            .as_ref()
            .map(|s| s.name.as_str())
            .unwrap_or_default()],
        controls::view(&model.controls).map_msg(Msg::ControlsMsg),
        canvas![
            &model.canvasser.el_ref(),
            attrs![
                At::Width => px(2200),
                At::Height => px(2000),
            ],
        ],
        // if let Some(radar) = &model.radar {
        //     format!(
        //         "data processed for {}, {}. {} trips",
        //         radar.day,
        //         radar.geometry.start_time,
        //         radar.trips.len()
        //     )
        // } else {
        //     format!("data received, {} stops", data.stops().count())
        // }
    ]
    // } else {
    //     div!["Data not loaded"]
    // }
}
