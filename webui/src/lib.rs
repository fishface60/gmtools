#![allow(clippy::single_component_path_imports)]

use url::Url;
use wasm_bindgen::prelude::*;
use yew::{
    self, html,
    services::{
        websocket::{WebSocketService, WebSocketStatus, WebSocketTask},
        ConsoleService,
    },
    Component, ComponentLink, Html, ShouldRender,
};

pub struct Model {
    agent_sock: Option<WebSocketTask>,
    clicked: bool,
    link: ComponentLink<Self>,
}

pub enum Msg {
    AgentSockConnected,
    AgentSockDisconnected,
    AgentSockReceived(String),
    Click,
    Ignore,
    Init,
}

impl Component for Model {
    type Message = Msg;
    type Properties = ();

    fn create(_: Self::Properties, link: ComponentLink<Self>) -> Self {
        link.send_message(Msg::Init);
        Model {
            agent_sock: None,
            clicked: false,
            link,
        }
    }

    fn change(&mut self, _: Self::Properties) -> bool {
        false
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        match msg {
            Msg::Init => {
                let window = if let Some(window) = web_sys::window() {
                    window
                } else {
                    ConsoleService::error(
                        format!("Failed to get window object").as_str(),
                    );
                    return false;
                };
                let href = if let Ok(href) = window.location().href() {
                    href
                } else {
                    ConsoleService::error(
                        format!("Failed to get window url").as_str(),
                    );
                    return false;
                };
                let url = if let Ok(url) = Url::parse(&href) {
                    url
                } else {
                    ConsoleService::error(
                        format!("Window url failed to parse").as_str(),
                    );
                    return false;
                };
                let mut agentaddr = None;
                for (k, v) in url.query_pairs() {
                    if k == "agentaddr" {
                        agentaddr = Some(v);
                        break;
                    }
                }
                let agentaddr = if let Some(agentaddr) = agentaddr {
                    agentaddr
                } else {
                    ConsoleService::error(
                        format!("url did not include agentaddr").as_str(),
                    );
                    return false;
                };
                let ws_url = format!("ws://{}", agentaddr);
                let cbout = self.link.callback(|data| match data {
                    Ok(data) => Msg::AgentSockReceived(data),
                    _ => Msg::Ignore,
                });
                let cbnot = self.link.callback(|event| match event {
                    WebSocketStatus::Opened => Msg::AgentSockConnected,
                    WebSocketStatus::Closed => Msg::AgentSockDisconnected,
                    WebSocketStatus::Error => {
                        // TODO: Some errors don't result in disconnection
                        // e.g. sending a message before connection established
                        // How can we distinguish which these are?
                        Msg::AgentSockDisconnected
                    }
                });

                if let Ok(ws) =
                    WebSocketService::connect_text(&ws_url, cbout, cbnot.into())
                {
                    self.agent_sock = Some(ws);
                } else {
                    ConsoleService::error(
                        format!("Failed to connect to web socket").as_str(),
                    );
                };
                false
            }
            Msg::Click => {
                self.clicked = true;
                true
            }
            Msg::Ignore => false,
            Msg::AgentSockReceived(s) => {
                ConsoleService::info(&s);
                false
            }
            Msg::AgentSockConnected => {
                if let Some(ws) = &mut self.agent_sock {
                    ws.send(Ok("src/main.rs".to_string()));
                    ConsoleService::log(format!("Sent path to agent").as_str());
                } else {
                    ConsoleService::error(
                        format!("Agent socket dropped before connect").as_str(),
                    );
                }
                false
            }
            Msg::AgentSockDisconnected => {
                ConsoleService::log(
                    format!("Agent socket disconnected").as_str(),
                );
                self.agent_sock = None;
                false
            }
        }
    }

    fn view(&self) -> Html {
        html! {
            <div>
                <button onclick=self.link.callback(|_| Msg::Click)>{ "Click ( wasm-pack )" }</button>
                <p>{format!("Has been clicked: {}", self.clicked)}</p>
            </div>
        }
    }
}

#[wasm_bindgen(start)]
pub fn main() -> Result<(), JsValue> {
    yew::start_app::<Model>();
    Ok(())
}
