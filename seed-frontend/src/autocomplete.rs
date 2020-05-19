use seed::prelude::*;
use seed::*;
use seed::{error, fetch};
use serde::Deserialize;
use web_sys::{Element, HtmlInputElement};

#[derive(Default)]
pub struct Model {
    input_ref: ElRef<HtmlInputElement>,
    search: String,
    suggestions: Vec<StationSuggestion>,
    is_open: bool,
    highlighted_index: Option<usize>,
    ignore_blur: bool,
    ignore_focus: bool,
}

#[derive(Deserialize, Clone)]
pub struct StationSuggestion {
    pub stop_id: u64,
    pub name: String,
}

pub enum Msg {
    Focus,
    Blur,
    KeyDown(web_sys::KeyboardEvent),
    InputClick(web_sys::MouseEvent),
    SuggestionClick(usize),
    SuggestionHover(usize),
    Input(String),
    Set(String),
    Select(StationSuggestion),
    SuggestionsFetched(Result<Vec<StationSuggestion>, LoadError>),
    SetIgnoreBlur(bool),
}

pub fn update(msg: Msg, model: &mut Model, orders: &mut impl Orders<Msg>) {
    match msg {
        Msg::Set(_) => panic!("autocomplete::Msg::Set should be handled by parent"),
        Msg::Select(_) => panic!("autocomplete::Msg::Select should be handled by parent"),

        Msg::Focus => {
            if model.ignore_focus {
                model.ignore_focus = false;
                // const { x, y } = this._scrollOffset
                // this._scrollOffset = null
                // // Focus will cause the browser to scroll the <input> into view.
                // // This can cause the mouse coords to change, which in turn
                // // could cause a new highlight to happen, cancelling the click
                // // event (when selecting with the mouse)
                // window.scrollTo(x, y)
                // // Some browsers wait until all focus event handlers have been
                // // processed before scrolling the <input> into view, so let's
                // // scroll again on the next tick to ensure we're back to where
                // // the user was before focus was lost. We could do the deferred
                // // scroll only, but that causes a jarring split second jump in
                // // some browsers that scroll before the focus event handlers
                // // are triggered.
                // clearTimeout(this._scrollTimer)
                // this._scrollTimer = setTimeout(() => {
                //     this._scrollTimer = null
                //     window.scrollTo(x, y)
                // }, 0)
                return;
            }
            // TODO handling for focus causing a scroll which could cause a click to be cancelled
            model.is_open = true;
        }

        Msg::Blur => {
            if model.ignore_blur {
                model.ignore_focus = true;
                // this._scrollOffset = getScrollOffset()
                model.input_ref.get().unwrap().focus().unwrap();
                return;
            }
            model.is_open = false;
            model.highlighted_index = None;
        }

        Msg::SetIgnoreBlur(value) => model.ignore_blur = value,

        Msg::KeyDown(kb_ev) => {
            match kb_ev.key().as_str() {
                "ArrowDown" => {
                    kb_ev.prevent_default();
                    if model.suggestions.is_empty() {
                        return;
                    }
                    let index = model.highlighted_index.map(|i| i + 1).unwrap_or(0);
                    if index < model.suggestions.len() {
                        model.highlighted_index = Some(index);
                        model.is_open = true;
                    }
                }
                "ArrowUp" => {
                    kb_ev.prevent_default();
                    if model.suggestions.is_empty() {
                        return;
                    }
                    let index = model.highlighted_index.unwrap_or(model.suggestions.len());
                    if index > 0 {
                        model.highlighted_index = Some(index - 1);
                        model.is_open = true;
                    }
                }
                "Enter" => {
                    // Key code 229 is used for selecting items from character selectors (Pinyin, Kana, etc)
                    if kb_ev.key_code() != 13 {
                        return;
                    }
                    // // In case the user is currently hovering over the menu
                    model.ignore_blur = false;
                    if !model.is_open {
                        // menu is closed so there is no selection to accept -> do nothing
                        return;
                    } else if let Some(highlighted_index) = model.highlighted_index {
                        // text entered + menu item has been highlighted + enter is hit -> update value to that of selected menu item, close the menu
                        kb_ev.prevent_default();
                        let item = &model.suggestions[highlighted_index];
                        model.is_open = false;
                        model.highlighted_index = None;
                        // //this.refs.input.focus() // TODO: file issue
                        // this.refs.input.setSelectionRange(
                        //     value.length,
                        //     value.length
                        // )
                        model.search = item.name.clone();
                        orders.send_msg(Msg::Select(item.clone()));
                    } else {
                        // input has focus but no menu item is selected + enter is hit -> close the menu, highlight whatever's in input
                        model.is_open = false;
                        // this.refs.input.select()
                    }
                }
                "Escape" => {
                    // In case the user is currently hovering over the menu
                    model.ignore_blur = false;
                    model.highlighted_index = None;
                    model.is_open = false;
                }
                "Tab" => {
                    // In case the user is currently hovering over the menu
                    model.ignore_blur = false;
                }
                _ => {
                    model.is_open = true;
                }
            }
        }

        Msg::InputClick(_mouse_ev) => {
            let element = model.input_ref.get().unwrap();
            if element
                .owner_document()
                .and_then(|doc| doc.active_element())
                .map(|active_element| active_element == element.into())
                .unwrap_or_default()
            {
                model.is_open = true;
            }
        }

        Msg::SuggestionHover(idx) => {
            model.highlighted_index = Some(idx);
        }

        Msg::SuggestionClick(idx) => {
            let item = &model.suggestions[idx];
            model.search = item.name.clone();
            model.ignore_blur = false;
            model.is_open = false;
            model.highlighted_index = None;
            orders.send_msg(Msg::Select(item.clone()));
        }

        Msg::Input(value) => {
            if value.len() >= 3 {
                orders.perform_cmd(
                    request(format!("/searchStation/{}", value)).map(Msg::SuggestionsFetched),
                );
            }
            model.search = value;
        }

        Msg::SuggestionsFetched(Ok(data)) => {
            model.suggestions = data;
        }

        Msg::SuggestionsFetched(Err(fail_reason)) => {
            error!(format!(
                "Fetch error - Fetching repository info failed - {:#?}",
                fail_reason
            ));
            orders.skip();
        }
    }
}

fn get_computed_style_float(
    style_declaration: &web_sys::CssStyleDeclaration,
    key: &str,
) -> Option<f64> {
    fn parse(value: String) -> Option<f64> {
        value.parse().ok()
    }
    style_declaration
        .get_property_value(key)
        .ok()
        .and_then(parse)
}

pub fn view(model: &Model) -> Node<Msg> {
    let mut menu_style = if let Some(node) = model.input_ref.get() {
        let node: Element = node.into();
        let rect = node.get_bounding_client_rect();
        let computed_style = window().get_computed_style(&node).unwrap().unwrap();
        let margin_bottom = get_computed_style_float(&computed_style, "marginBottom").unwrap_or(0.);
        let margin_left = get_computed_style_float(&computed_style, "marginLeft").unwrap_or(0.);
        let margin_right = get_computed_style_float(&computed_style, "marginRight").unwrap_or(0.);
        style! {
            St::Left => format!("{}px", rect.left() + margin_left),
            St::Top => format!("{}px", rect.bottom() + margin_bottom),
            St::MinWidth => format!("{}px", rect.width() + margin_left + margin_right),
        }
    } else {
        style! {}
    };
    menu_style.merge(style! {
      St::BorderRadius => "3px",
      St::BoxShadow => "0 2px 12px rgba(0, 0, 0, 0.1)",
      St::Background => "rgba(255, 255, 255, 0.9)",
      St::Padding => "2px 0",
      St::FontSize => "90%",
      St::Position => "fixed",
      St::Overflow => "auto",
      St::MaxHeight => "50%", // TODO: don't cheat, let it flow to the bottom
    });
    div![
        input![
            el_ref(&model.input_ref),
            attrs! {
                At::Type => "search",
                At::List => "station-suggestions",
                At::Value => &model.search,
            },
            input_ev(Ev::Input, Msg::Input),
            input_ev(Ev::Change, Msg::Set),
            ev(Ev::Focus, |_| Msg::Focus),
            input_ev(Ev::Blur, |_| Msg::Blur),
            keyboard_ev(Ev::KeyDown, Msg::KeyDown),
            mouse_ev(Ev::Click, Msg::InputClick),
        ],
        if model.is_open {
            div![
                menu_style,
                attrs! {
                    At::Id => "station-suggestions",
                },
                model.suggestions.iter().enumerate().map(|(idx, suggestion)| div![
                    // attrs! {
                    //     At::Value => suggestion.stop_id,
                    // },
                    style! {
                        St::Background => if Some(idx) == model.highlighted_index { "lightgray" } else { "white" },
                        St::Cursor => "default",
                    },
                    &suggestion.name,
                    ev(Ev::MouseEnter, move |_| Msg::SuggestionHover(idx)),
                    ev(Ev::Click, move |_| Msg::SuggestionClick(idx)),
                ]),
                ev(Ev::TouchStart, |_| Msg::SetIgnoreBlur(true)),
                ev(Ev::MouseEnter, |_| Msg::SetIgnoreBlur(true)),
                ev(Ev::MouseLeave, |_| Msg::SetIgnoreBlur(false)),
            ]
        } else {
            div![]
        },
    ]
}

async fn request(url: String) -> Result<Vec<StationSuggestion>, LoadError> {
    let response = fetch::fetch(url).await?;
    Ok(response.json().await?)
}

#[derive(Debug)]
pub enum LoadError {
    FetchError(fetch::FetchError),
    RMPError(rmp_serde::decode::Error),
}

impl From<fetch::FetchError> for LoadError {
    fn from(error: fetch::FetchError) -> LoadError {
        Self::FetchError(error)
    }
}

impl From<rmp_serde::decode::Error> for LoadError {
    fn from(error: rmp_serde::decode::Error) -> LoadError {
        Self::RMPError(error)
    }
}
