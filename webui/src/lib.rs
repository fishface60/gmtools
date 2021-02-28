#![allow(clippy::single_component_path_imports)]
#![recursion_limit = "512"]

use url::Url;
use wasm_bindgen::prelude::*;
use web_sys::{
    FocusEvent, HtmlInputElement, HtmlOptionElement, HtmlSelectElement,
};
use yew::{
    self, html,
    services::{
        websocket::{WebSocketService, WebSocketStatus, WebSocketTask},
        ConsoleService,
    },
    ChangeData, Component, ComponentLink, Html, NodeRef, ShouldRender,
};

use gmtool_common::{FileEntry, GCSAgentMessage, WebUIMessage};

pub struct Model {
    agent_sock: Option<WebSocketTask>,
    dir_path_element: NodeRef,
    entries_element: NodeRef,
    link: ComponentLink<Self>,
}

pub enum Msg {
    AgentSockDisconnected,
    AgentSockReceived(GCSAgentMessage),
    DirectoryEntrySelected(FileEntry),
    DirectoryPathSubmitted,
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
            dir_path_element: NodeRef::default(),
            entries_element: NodeRef::default(),
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
                    ConsoleService::error("Failed to get window object");
                    return false;
                };
                let href = if let Ok(href) = window.location().href() {
                    href
                } else {
                    ConsoleService::error("Failed to get window url");
                    return false;
                };
                let url = if let Ok(url) = Url::parse(&href) {
                    url
                } else {
                    ConsoleService::error("Window url failed to parse");
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
                    ConsoleService::error("url did not include agentaddr");
                    return false;
                };
                let ws_url = format!("ws://{}", agentaddr);
                let cbout = self.link.callback(|data| match data {
                    Ok(data) => {
                        let s: Vec<u8> = data;
                        match bincode::deserialize(&s) {
                            Ok(message) => Msg::AgentSockReceived(message),
                            Err(e) => {
                                ConsoleService::error(&format!("{}", e));
                                Msg::Ignore
                            }
                        }
                    }
                    _ => Msg::Ignore,
                });
                let cbnot = self.link.callback(|event| match event {
                    WebSocketStatus::Opened => Msg::Ignore,
                    WebSocketStatus::Closed => Msg::AgentSockDisconnected,
                    WebSocketStatus::Error => {
                        // TODO: Some errors don't result in disconnection
                        // e.g. sending a message before connection established
                        // How can we distinguish which these are?
                        Msg::AgentSockDisconnected
                    }
                });

                if let Ok(ws) =
                    WebSocketService::connect_binary(&ws_url, cbout, cbnot)
                {
                    self.agent_sock = Some(ws);
                } else {
                    ConsoleService::error("Failed to connect to web socket");
                };
                false
            }
            Msg::AgentSockReceived(m) => {
                ConsoleService::debug(&format!("Socket message {:?}", m));
                match &m {
                    GCSAgentMessage::RequestChDirResult(result) => {
                        ConsoleService::debug(&format!("{:?}", m));
                        if let Err(text) = result {
                            ConsoleService::error(&text);
                        }
                    }
                    GCSAgentMessage::RequestWatchResult(result) => {
                        ConsoleService::debug(&format!("{:?}", m));
                        if let Err(text) = result {
                            ConsoleService::error(&text);
                        }
                    }
                    GCSAgentMessage::FileChangeNotification(path) => {
                        ConsoleService::log(&path);
                    }
                    GCSAgentMessage::DirectoryChangeNotification(msg) => {
                        match msg {
                            Ok((path, entries)) => {
                                ConsoleService::log(&path);
                                self.dir_path_element
                                    .cast::<HtmlInputElement>()
                                    .expect("dir_path instantiated")
                                    .set_value(&path);
                                let entries_element = self
                                    .entries_element
                                    .cast::<HtmlSelectElement>()
                                    .expect("entries select intstantiated");
                                for _ in 0..entries_element.length() {
                                    entries_element.remove_with_index(0);
                                }

                                let mut text = String::from("../");
                                let option =
                                    HtmlOptionElement::new_with_text_and_value(
                                        &text,
                                        &serde_json::to_string(
                                            &FileEntry::Directory(
                                                String::from(".."),
                                            ),
                                        )
                                        .unwrap(),
                                    )
                                    .unwrap();
                                entries_element
                                    .add_with_html_option_element(&option)
                                    .unwrap();

                                for entry in entries {
                                    match entry {
                                        FileEntry::GCSFile(ref name) => {
                                            let option =
                                                HtmlOptionElement::new_with_text_and_value(
                                                    name, &serde_json::to_string(&entry).unwrap(),
                                                )
                                                .unwrap();
                                            entries_element
                                                .add_with_html_option_element(
                                                    &option,
                                                )
                                                .unwrap();
                                            ConsoleService::log(&format!(
                                                "File {}",
                                                name
                                            ));
                                        }
                                        FileEntry::Directory(ref name) => {
                                            text.clear();
                                            text.push_str(&name);
                                            text.push('/');
                                            let option =
                                                HtmlOptionElement::new_with_text_and_value(
                                                    &text, &serde_json::to_string(&entry).unwrap(),
                                                )
                                                .unwrap();
                                            entries_element
                                                .add_with_html_option_element(
                                                    &option,
                                                )
                                                .unwrap();
                                            ConsoleService::log(&format!(
                                                "Directory {}",
                                                name
                                            ));
                                        }
                                    }
                                }
                            }
                            Err(s) => {
                                ConsoleService::error(&s);
                            }
                        }
                    }
                };
                false
            }
            Msg::AgentSockDisconnected => {
                ConsoleService::log("Agent socket disconnected");
                self.agent_sock = None;
                false
            }
            Msg::DirectoryEntrySelected(entry) => {
                let ws = if let Some(ws) = &mut self.agent_sock {
                    ws
                } else {
                    ConsoleService::error(
                        "Selected dirent without agent socket",
                    );
                    return false;
                };
                let msg = match entry {
                    FileEntry::Directory(path) => {
                        WebUIMessage::RequestChDir(path)
                    }
                    FileEntry::GCSFile(path) => {
                        WebUIMessage::RequestWatch(path)
                    }
                };
                match bincode::serialize(&msg) {
                    Ok(bytes) => ws.send_binary(Ok(bytes)),
                    Err(e) => ConsoleService::error(&format!("{:?}", e)),
                }
                false
            }
            Msg::DirectoryPathSubmitted => {
                ConsoleService::info("DirectoryPathSubmitted");
                let ws = if let Some(ws) = &mut self.agent_sock {
                    ws
                } else {
                    ConsoleService::error(
                        "Submitted directory path without agent socket",
                    );
                    return false;
                };
                let path = self
                    .dir_path_element
                    .cast::<HtmlInputElement>()
                    .unwrap()
                    .value();
                match bincode::serialize(&WebUIMessage::RequestChDir(path)) {
                    Ok(bytes) => ws.send_binary(Ok(bytes)),
                    Err(e) => ConsoleService::error(&format!("{:?}", e)),
                }
                false
            }
            Msg::Ignore => false,
        }
    }

    fn view(&self) -> Html {
        html! {
            <div>
                <table>
                  <tr>
                    <th>{"Directory"}</th>
                    <td>
                      <form
                       onsubmit=self.link.callback(|evt: FocusEvent| {
                         evt.prevent_default();
                         ConsoleService::info(&"Submit");
                         Msg::DirectoryPathSubmitted
                       })>
                        <input ref=self.dir_path_element.clone()/>
                      </form>
                    </td>
                  </tr>
                  <tr>
                    <th>{"Entries"}</th>
                    <td>
                      <select ref=self.entries_element.clone()  multiple=true
                       style="width: 100%;"
                       onchange=self.link.callback(|evt: ChangeData| match evt {
                         ChangeData::Select(ref select_element) => {
                             ConsoleService::info(&format!("{:?}", evt));
                             ConsoleService::info(&format!("{:?}", select_element.value()));
                             let entry = serde_json::from_str(&select_element.value()).unwrap();
                             Msg::DirectoryEntrySelected(entry)
                         }
                         _ => Msg::Ignore
                       })>
                      </select>
                    </td>
                  </tr>
                </table>
            </div>
        }
    }
}

#[wasm_bindgen(start)]
pub fn main() -> Result<(), JsValue> {
    yew::start_app::<Model>();
    Ok(())
}
