use seed::{prelude::*, *};
use seed_autocomplete::{self as autocomplete, ViewBuilder};
use serde::{Deserialize, Serialize};
use std::fmt;

pub struct Model {
    pub params: Params,
    station_autocomplete: autocomplete::Model<Msg, StationSuggestion>,
    station_input: String,
    enable_url_routing: bool,
}

impl Model {
    pub fn init(mut url: Url, orders: &mut impl Orders<Msg>) -> Model {
        let first_part = url.next_path_part();
        let mut station_name = None;
        let enable_url_routing;
        if let Some(part) = first_part {
            if !part.ends_with(".html") {
                orders.perform_cmd(
                    request(format!("/searchStation/{}", part)).map(Msg::SuggestionsFetched),
                );
                enable_url_routing = true;
                station_name = Some(part);
            } else {
                enable_url_routing = false;
            }
        } else {
            enable_url_routing = true
        }

        Model {
            station_autocomplete: autocomplete::Model::new(Msg::StationSuggestions)
                .on_selection(|_| Some(Msg::AStationSelected))
                .on_input_change(|s| Some(Msg::StationInputChanged(s.to_owned()))),
            station_input: station_name.unwrap_or_default().to_owned(),
            params: Params::from(&url),
            enable_url_routing,
        }
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct Params {
    pub flags: Flags,
    pub station_selection: Option<StationSuggestion>,
}

impl Params {
    fn url(&self) -> String {
        let path = self
            .station_selection
            .as_ref()
            .map_or("", |station| &station.name);
        let url_query = serde_urlencoded::to_string(&self.flags).expect("serialize flags");
        format!("/{}?{}", path, url_query)
    }
}

impl From<&Url> for Params {
    fn from(url: &Url) -> Self {
        Params {
            flags: serde_urlencoded::from_str(&url.search().to_string()).unwrap_or_default(),
            station_selection: None,
        }
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
#[serde(default = "Default::default")]
#[allow(clippy::struct_excessive_bools)]
pub struct Flags {
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub show_sbahn: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub show_ubahn: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub show_bus: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub show_tram: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub show_regional: bool,
}

#[derive(Debug)]
pub enum Msg {
    SetShowSBahn(String),
    SetShowUBahn(String),
    SetShowBus(String),
    SetShowTram(String),
    SetShowRegional(String),
    StationSuggestions(autocomplete::Msg),
    AStationSelected,
    StationSelected(StationSuggestion),
    StationInputChanged(String),
    SuggestionsFetched(Result<Vec<StationSuggestion>, LoadError>),
}

pub fn view(model: &Model) -> Vec<Node<Msg>> {
    nodes![
        span!["Search a station in Berlin :"],
        model
            .station_autocomplete
            .view()
            .with_input_attrs(attrs! {
                At::Value => model.station_input,
            })
            .into_nodes(),
        checkbox(
            "show-sbahn",
            "Show SBahn",
            model.params.flags.show_sbahn,
            &Msg::SetShowSBahn
        ),
        checkbox(
            "show-ubahn",
            "Show UBahn",
            model.params.flags.show_ubahn,
            &Msg::SetShowUBahn
        ),
        checkbox(
            "show-bus",
            "Show Bus",
            model.params.flags.show_bus,
            &Msg::SetShowBus
        ),
        checkbox(
            "show-tram",
            "Show Tram",
            model.params.flags.show_tram,
            &Msg::SetShowTram
        ),
        checkbox(
            "show-regional",
            "Show Regional",
            model.params.flags.show_regional,
            &Msg::SetShowRegional
        ),
    ]
}

pub fn update(msg: Msg, model: &mut Model, orders: &mut impl Orders<Msg>) -> bool {
    let mut params_changed = true;
    let params = &mut model.params;
    match msg {
        Msg::SetShowSBahn(_value) => params.flags.show_sbahn = !params.flags.show_sbahn,
        Msg::SetShowUBahn(_value) => params.flags.show_ubahn = !params.flags.show_ubahn,
        Msg::SetShowBus(_value) => params.flags.show_bus = !params.flags.show_bus,
        Msg::SetShowTram(_value) => params.flags.show_tram = !params.flags.show_tram,
        Msg::SetShowRegional(_value) => params.flags.show_regional = !params.flags.show_regional,
        Msg::AStationSelected => {
            if let Some(station) = model.station_autocomplete.get_selection().cloned() {
                orders.send_msg(Msg::StationSelected(station));
            }
        }
        Msg::StationSelected(station) => {
            params.station_selection = Some(station);
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
            model.station_autocomplete.update(msg, orders);
            // params has not changed
            params_changed = false;
        }
        Msg::SuggestionsFetched(Ok(suggestions)) => {
            params_changed = false;
            // automatically select a suggestion if it matches the url route
            if model.enable_url_routing {
                if let Some(route_station) = Url::current().next_path_part() {
                    if let Some(matching_suggestion) =
                        suggestions.iter().find(|s| s.name == route_station)
                    {
                        params.station_selection = Some(matching_suggestion.clone());
                        params_changed = true;
                    }
                }
            }
            model.station_autocomplete.set_suggestions(suggestions);
        }

        Msg::SuggestionsFetched(Err(fail_reason)) => {
            error!(format!(
                "Fetch error - Fetching repository info failed - {:#?}",
                fail_reason
            ));
            orders.skip();
            // params has not changed
            params_changed = false;
        }
    }
    if params_changed {
        let old_params: Option<Params> = util::history()
            .state()
            .ok()
            .and_then(|js_value| js_value.into_serde().ok());
        if !old_params.iter().any(|old_params| params == old_params) {
            // params changed, push to history
            if model.enable_url_routing {
                util::history()
                    .replace_state_with_url(
                        &JsValue::from_serde(params).expect("Convert params to JS"),
                        "",
                        Some(&params.url()),
                    )
                    .expect("Problems pushing state");
            }
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

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
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
