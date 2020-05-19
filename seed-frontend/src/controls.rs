use crate::autocomplete;
use seed::{prelude::*, *};
use serde::Deserialize;

#[derive(Default)]
pub struct Model {
    pub params: Params,
    pub station_autocomplete: autocomplete::Model,
}

#[derive(Default, Clone)]
pub struct Params {
    pub show_stations: bool,
    pub animate: bool,
    pub show_sbahn: bool,
    pub show_ubahn: bool,
    pub show_bus: bool,
    pub show_tram: bool,
    pub show_regional: bool,
    pub station_selection: Option<autocomplete::StationSuggestion>,
}

pub fn view(model: &Model) -> Vec<Node<Msg>> {
    nodes![
        span!["Search a station in Berlin :"],
        autocomplete::view(&model.station_autocomplete).map_msg(Msg::StationSuggestions),
        checkbox(
            "show-stations",
            "Show Stations",
            model.params.show_stations,
            &Msg::SetShowStations
        ),
        checkbox("animate", "Animate", model.params.animate, &Msg::SetAnimate),
        checkbox(
            "show-sbahn",
            "Show SBahn",
            model.params.show_sbahn,
            &Msg::SetShowSBahn
        ),
        checkbox(
            "show-ubahn",
            "Show UBahn",
            model.params.show_ubahn,
            &Msg::SetShowUBahn
        ),
        checkbox(
            "show-bus",
            "Show Bus",
            model.params.show_bus,
            &Msg::SetShowBus
        ),
        checkbox(
            "show-tram",
            "Show Tram",
            model.params.show_tram,
            &Msg::SetShowTram
        ),
        checkbox(
            "show-regional",
            "Show Regional",
            model.params.show_regional,
            &Msg::SetShowRegional
        ),
    ]
}

pub enum Msg {
    SetShowStations(String),
    SetAnimate(String),
    SetShowSBahn(String),
    SetShowUBahn(String),
    SetShowBus(String),
    SetShowTram(String),
    SetShowRegional(String),
    StationSuggestions(autocomplete::Msg),
}

pub fn update(msg: Msg, model: &mut Model, orders: &mut impl Orders<Msg>) -> bool {
    let params = &mut model.params;
    match msg {
        Msg::SetShowStations(_value) => params.show_stations = !params.show_stations,
        Msg::SetAnimate(_value) => params.animate = !params.animate,
        Msg::SetShowSBahn(_value) => params.show_sbahn = !params.show_sbahn,
        Msg::SetShowUBahn(_value) => params.show_ubahn = !params.show_ubahn,
        Msg::SetShowBus(_value) => params.show_bus = !params.show_bus,
        Msg::SetShowTram(_value) => params.show_tram = !params.show_tram,
        Msg::SetShowRegional(_value) => params.show_regional = !params.show_regional,
        Msg::StationSuggestions(autocomplete::Msg::Set(value)) => {} // we ignore set as we require that a suggestion is selected
        Msg::StationSuggestions(autocomplete::Msg::Select(value)) => {
            params.station_selection.replace(value);
        }
        Msg::StationSuggestions(msg) => {
            autocomplete::update(
                msg,
                &mut model.station_autocomplete,
                &mut orders.proxy(Msg::StationSuggestions),
            );
            // params has not changed
            return false;
        }
    }
    true
}

fn checkbox<M>(
    name: &'static str,
    label: &'static str,
    value: bool,
    event: &'static M,
) -> Vec<Node<Msg>>
where
    M: FnOnce(String) -> Msg + Copy,
{
    vec![
        input![
            attrs! {
                At::Type => "checkbox",
                At::Checked => value.as_at_value(),
                At::Name => name,
            },
            input_ev(Ev::Input, *event)
        ],
        label![
            attrs! {
                At::For => name
            },
            label
        ],
    ]
}
