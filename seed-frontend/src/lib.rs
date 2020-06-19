use enclose::enclose;
use radar_search::search_data::*;
use radar_search::search_data_sync::*;
use radar_search::time::*;
use seed::{prelude::*, *};
use std::cell::RefCell;

mod canvasser;
mod controls;
mod radar;
mod scheduler;
mod sync;

#[cfg(not(feature = "storybook"))]
#[wasm_bindgen(start)]
pub fn render() {
    App::start("app", init, update, view);
}

fn init(url: Url, orders: &mut impl Orders<Msg>) -> Model {
    orders.after_next_render(|_| Msg::FirstRender);

    Model {
        scheduler: RefCell::new(scheduler::Scheduler::new()),
        sync: Default::default(),
        canvasser: radar::init(None),
        controls: controls::Model::init(url, &mut orders.proxy(Msg::ControlsMsg)),
    }
}

struct Model {
    scheduler: RefCell<scheduler::Scheduler>,
    sync: sync::Model<GTFSData>,

    canvasser: canvasser::App<Option<radar::Radar>, f64>,
    controls: controls::Model,
}

enum Msg {
    FirstRender,
    /// When a user changes a control
    ControlsMsg(controls::Msg),

    SyncMsg(sync::Msg<GTFSData, GTFSSyncIncrement>),
    Search,
    SearchExpires,
    LoadDataAhead(Time),
}

fn update(msg: Msg, model: &mut Model, orders: &mut impl Orders<Msg>) {
    match msg {
        Msg::ControlsMsg(msg) => {
            if controls::update(
                msg,
                &mut model.controls,
                &mut orders.proxy(Msg::ControlsMsg),
            ) {
                orders.send_msg(Msg::Search);
                orders.send_msg(Msg::SyncMsg(sync::Msg::FetchData));
            }
        }

        Msg::FirstRender => {
            if model.sync.never_requested() {
                orders.send_msg(Msg::SyncMsg(sync::Msg::FetchData));
            }
        }

        Msg::SyncMsg(msg) => {
            let (_day, start_time) = radar::day_time(&js_sys::Date::new_0());
            sync_data(msg, model, orders, start_time);
        }

        Msg::LoadDataAhead(start_time) => {
            sync_data(sync::Msg::FetchData, model, orders, start_time);
        }

        Msg::Search => {
            if let Some(data) = model.sync.get() {
                if let Some(origin) = model
                    .controls
                    .params
                    .station_selection
                    .as_ref()
                    .and_then(|suggestion| data.get_stop(&suggestion.stop_id))
                {
                    let previous_expires_timestamp = model
                        .canvasser
                        .model()
                        .as_ref()
                        .map(|radar| radar.expires_timestamp);
                    let result = radar::search(data, origin, &model.controls.params);
                    let expires_date = js_sys::Date::new_0();
                    expires_date.set_time(result.expires_timestamp as f64);
                    let next_search_time = radar::day_time(&expires_date).1;
                    if previous_expires_timestamp != Some(result.expires_timestamp) {
                        let msg_mapper = orders.msg_mapper();
                        schedule_msg(
                            &model.scheduler.borrow_mut(),
                            orders.clone_app(),
                            result.expires_timestamp - 15_000,
                            msg_mapper(Msg::LoadDataAhead(next_search_time)),
                        );
                        schedule_msg(
                            &model.scheduler.borrow_mut(),
                            orders.clone_app(),
                            result.expires_timestamp,
                            msg_mapper(Msg::SearchExpires),
                        );
                    }
                    model.canvasser.model_mut().replace(result);
                }
            }
        }

        Msg::SearchExpires => {
            let date = js_sys::Date::new_0();
            let expires_timestamp = model.canvasser.model().as_ref().unwrap().expires_timestamp;
            // check whether it is actually expired, it may have been updated before this message was scheduled
            if date.value_of() as u64 > expires_timestamp {
                orders.send_msg(Msg::Search);
            }
        }
    }
}

fn view(model: &Model) -> Node<Msg> {
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
            model.canvasser.canvas_ref(),
            attrs![
                At::Width => px(2200),
                At::Height => px(2000),
            ],
        ],
        if let Some(data) = model.sync.get() {
            let radar = model.canvasser.model();
            div![if let Some(radar) = radar.as_ref() {
                format!(
                    "data processed for {}, {}. {} trips",
                    radar.day, radar.geometry.start_time, radar.trip_count,
                )
            } else {
                format!("data received, {} stops", data.stops().count())
            },]
        } else {
            div!["Data not loaded"]
        }
    ]
}

fn schedule_msg<Ms, Mdl, INodes: IntoNodes<Ms> + 'static>(
    scheduler: &scheduler::Scheduler,
    app: App<Ms, Mdl, INodes>,
    timestamp: u64,
    msg: Ms,
) {
    let f = enclose!((app => s) move || s.update(msg));
    scheduler.schedule(timestamp, f);
}

#[derive(serde::Serialize)]
#[allow(clippy::struct_excessive_bools)]
struct Params {
    ubahn: bool,
    sbahn: bool,
    bus: bool,
    tram: bool,
    regio: bool,
    start_time: Time,
    end_time: Time,
}

fn sync_data(
    msg: sync::Msg<GTFSData, GTFSSyncIncrement>,
    model: &mut Model,
    orders: &mut impl Orders<Msg>,
    start_time: Time,
) {
    let params = &model.controls.params;
    if let Some(station_selection) = &params.station_selection {
        let query = serde_urlencoded::to_string(Params {
            ubahn: params.flags.show_ubahn,
            sbahn: params.flags.show_sbahn,
            tram: params.flags.show_tram,
            regio: params.flags.show_regional,
            bus: params.flags.show_bus,
            start_time,
            end_time: start_time + Duration::minutes(40),
        })
        .unwrap();
        let url = format!(
            "/data/{}?{}",
            station_selection.name.replace(" ", "%20"),
            query
        );
        if sync::update(msg, &mut model.sync, url, &mut orders.proxy(Msg::SyncMsg)) {
            orders.send_msg(Msg::Search);
        }
    }
}

#[cfg(feature = "storybook")]
#[wasm_bindgen(start)]
pub fn start_storybook() {
    canvasser::animate::storybook::start();
}
