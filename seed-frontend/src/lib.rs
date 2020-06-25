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
        controls: controls::Model::init(url, &mut orders.proxy(Msg::ControlsComponent)),
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
    /// Messages for the controls component
    ControlsComponent(controls::Msg),

    SyncComponent(sync::Msg<GTFSData, GTFSSyncIncrement>),
    Search,
    SearchExpires,
    LoadDataAhead(Time),
}

fn update(msg: Msg, model: &mut Model, orders: &mut impl Orders<Msg>) {
    match msg {
        Msg::FirstRender => {
            if model.sync.never_requested() {
                orders.send_msg(Msg::SyncComponent(sync::Msg::FetchData));
            }
        }

        Msg::SyncComponent(msg) => {
            let (_day, start_time) = radar::day_time(&js_sys::Date::new_0());
            sync_data(msg, model, orders, start_time);
        }

        Msg::LoadDataAhead(start_time) => {
            sync_data(sync::Msg::FetchData, model, orders, start_time);
        }

        Msg::ControlsComponent(msg) => {
            if controls::update(
                msg,
                &mut model.controls,
                &mut orders.proxy(Msg::ControlsComponent),
            ) {
                orders.send_msg(Msg::Search);
                orders.send_msg(Msg::SyncComponent(sync::Msg::FetchData));
            }
        }

        Msg::Search => {
            if let Some(data) = model.sync.get() {
                let origin = model
                    .controls
                    .selected_station()
                    .and_then(|suggestion| data.get_stop(suggestion.stop_id));
                if let Some(origin) = origin {
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
            if date.value_of() > expires_timestamp as f64 {
                orders.send_msg(Msg::Search);
            }
        }
    }
}

fn view(model: &Model) -> Node<Msg> {
    let station_name = model
        .controls
        .selected_station()
        .map(|s| s.name.as_str())
        .unwrap_or_default();

    div![
        h2![&station_name],
        controls::view(&model.controls).map_msg(Msg::ControlsComponent),
        p![if let Some(data) = model.sync.get() {
            let radar = model.canvasser.model();
            if let Some(radar) = radar.as_ref() {
                format!(
                    "
                    The transit radar shows all the destinations you could reach within {} mins \
                    using the selected transport modes from the selected station, departing \
                    on a {} at {} and uses VBB's published timetables at {}.\
                ",
                    30,
                    radar.day,
                    radar.geometry.start_time,
                    data.timetable_start_date()
                )
            } else {
                format!("data received, {} stops", data.stops().count())
            }
        } else {
            "Data not loaded".to_owned()
        }],
        canvas![
            model.canvasser.canvas_ref(),
            attrs![
                At::Width => px(2200),
                At::Height => px(2000),
            ],
        ],
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
        if sync::update(
            msg,
            &mut model.sync,
            url,
            &mut orders.proxy(Msg::SyncComponent),
        ) {
            orders.send_msg(Msg::Search);
        }
    }
}

#[cfg(feature = "storybook")]
#[wasm_bindgen(start)]
pub fn start_storybook() {
    canvasser::animate::storybook::start();
}
