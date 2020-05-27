use seed::{prelude::*, *};
use seed_autocomplete::{self as autocomplete, ViewBuilder};
use serde::Deserialize;
use std::fmt;

pub struct Model {
    pub params: Params,
    station_autocomplete: autocomplete::Model<Msg, StationSuggestion>,
    station_input: String,
}

impl Default for Model {
    fn default() -> Self {
        Model {
            params: Params::default(),
            station_autocomplete: autocomplete::Model::new(Msg::StationSuggestions)
                .on_selection(|_| Some(Msg::StationSelected))
                .on_input_change(|s| Some(Msg::StationInputChanged(s.to_owned()))),
            station_input: "".to_owned(),
        }
    }
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
    pub station_selection: Option<StationSuggestion>,
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
    StationSelected,
    StationInputChanged(String),
    SuggestionsFetched(Result<Vec<StationSuggestion>, LoadError>),
}

pub fn view(model: &Model) -> Vec<Node<Msg>> {
    nodes![
        span!["Search a station in Berlin :"],
        model.station_autocomplete.view().with_input_attrs(attrs! {
            At::Value => model.station_input,
        }).into_nodes(),
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
        Msg::StationSelected => {
            params.station_selection = model.station_autocomplete.get_selection().cloned();
        }
        Msg::StationInputChanged(value) => {
            if value.len() >= 3 {
                orders.perform_cmd(
                    request(format!("/searchStation/{}", &value)).map(Msg::SuggestionsFetched),
                );
            }
            model.station_input = value;
        }
        Msg::StationSuggestions(msg) => {
            model.station_autocomplete.update(
                msg,
                orders,
            );
            // params has not changed
            return false;
        }
        Msg::SuggestionsFetched(Ok(data)) => {
            model.station_autocomplete.set_suggestions(data);
        }

        Msg::SuggestionsFetched(Err(fail_reason)) => {
            error!(format!(
                "Fetch error - Fetching repository info failed - {:#?}",
                fail_reason
            ));
            orders.skip();
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

#[derive(Deserialize, Clone)]
pub struct StationSuggestion {
    pub stop_id: u64,
    pub name: String,
}

impl fmt::Display for StationSuggestion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name)
    }
}

async fn request(url: String) -> Result<Vec<StationSuggestion>, LoadError> {
    let response = fetch::fetch(url).await?;
    Ok(response.json().await?)
}

#[derive(Debug)]
pub enum LoadError {
    FetchError(fetch::FetchError),
}

impl From<fetch::FetchError> for LoadError {
    fn from(error: fetch::FetchError) -> LoadError {
        Self::FetchError(error)
    }
}
