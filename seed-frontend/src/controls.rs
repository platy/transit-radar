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
    pub fn init(mut url: Url, orders: &mut impl Orders<Msg>) -> Self {
        let station_name: Option<String>;
        let params;
        let enable_url_routing = Self::should_enable_url_routing(url.clone());
        if enable_url_routing {
            station_name = url.next_path_part().map(str::to_owned);
            if let Some(station_name) = &station_name {
                orders.perform_cmd(
                    request(format!("/searchStation/{}", station_name))
                        .map(Msg::SuggestionsFetched),
                );
            }
            params = Params::from(&url);
        } else {
            params = Params::default();
            station_name = None;
        }

        Self {
            station_autocomplete: autocomplete::Model::new(Msg::StationSuggestions)
                .on_selection(|_| Some(Msg::AStationSelected))
                .on_input_change(|s| Some(Msg::StationInputChanged(s.to_owned()))),
            station_input: station_name.unwrap_or_default(),
            params,
            enable_url_routing,
        }
    }

    fn should_enable_url_routing(mut url: Url) -> bool {
        let first_part = url.next_path_part();
        if let Some(part) = first_part {
            !part.ends_with(".html")
        } else {
            true
        }
    }

    pub fn selected_station(&self) -> Option<&StationSuggestion> {
        self.params.station_selection.as_ref()
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
        Self {
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
        model.station_autocomplete.view().with_input_attrs(attrs! {
            At::Value => model.station_input,
        }),
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
            if !value.is_empty() {
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
            // automatically select a suggestion if it matches the url route (for a page load case where station is referenced and so a suggestion request is made for that initially)
            if model.enable_url_routing {
                if let Some(pre_selection) =
                    find_url_selection_in_suggestions(Url::current(), &suggestions)
                {
                    params.station_selection = Some(pre_selection);
                    params_changed = true;
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

/// Finds a suggestion in the slice whose name matches the first part of the url
fn find_url_selection_in_suggestions(
    mut url: Url,
    suggestions: &[StationSuggestion],
) -> Option<StationSuggestion> {
    if let Some(route_station) = url.next_path_part() {
        if let Some(matching_suggestion) = suggestions.iter().find(|s| s.name == route_station) {
            return Some(matching_suggestion.clone());
        }
    }
    None
}

/// Build a checkbox
fn checkbox<M, F>(
    name: &'static str,
    label: &'static str,
    value: bool,
    event: &'static F,
) -> Vec<Node<M>>
where
    F: FnOnce(String) -> M + Copy,
    M: 'static,
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
    fn from(error: fetch::FetchError) -> Self {
        Self::FetchError(error)
    }
}
