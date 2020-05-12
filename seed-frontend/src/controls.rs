use seed::{prelude::*, *};

#[derive(Default, Clone)]
pub struct Model {
    pub show_stations: bool,
    pub animate: bool,
    pub show_sbahn: bool,
    pub show_ubahn: bool,
    pub show_bus: bool,
    pub show_tram: bool,
    pub show_regional: bool,
}

pub fn view(model: &Model) -> Vec<Node<Msg>> {
    nodes![
        checkbox(
            "show-stations",
            "Show Stations",
            model.show_stations,
            &Msg::SetShowStations
        ),
        checkbox("animate", "Animate", model.animate, &Msg::SetAnimate),
        checkbox(
            "show-sbahn",
            "Show SBahn",
            model.show_sbahn,
            &Msg::SetShowSBahn
        ),
        checkbox(
            "show-ubahn",
            "Show UBahn",
            model.show_ubahn,
            &Msg::SetShowUBahn
        ),
        checkbox("show-bus", "Show Bus", model.show_bus, &Msg::SetShowBus),
        checkbox("show-tram", "Show Tram", model.show_tram, &Msg::SetShowTram),
        checkbox(
            "show-regional",
            "Show Regional",
            model.show_regional,
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
}

pub fn update(msg: Msg, model: &mut Model, _orders: &mut impl Orders<Msg>) {
    match msg {
        Msg::SetShowStations(_value) => model.show_stations = !model.show_stations,
        Msg::SetAnimate(_value) => model.animate = !model.animate,
        Msg::SetShowSBahn(_value) => model.show_sbahn = !model.show_sbahn,
        Msg::SetShowUBahn(_value) => model.show_ubahn = !model.show_ubahn,
        Msg::SetShowBus(_value) => model.show_bus = !model.show_bus,
        Msg::SetShowTram(_value) => model.show_tram = !model.show_tram,
        Msg::SetShowRegional(_value) => model.show_regional = !model.show_regional,
    }
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
