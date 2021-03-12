#![allow(clippy::single_component_path_imports, clippy::large_enum_variant)]
#![recursion_limit = "512"]

mod navlist;
mod sheetlist;
mod weakcomponentlink;

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

use gmtool_common::{FileEntry, GCSAgentMessage, GCSFile, WebUIMessage};
use navlist::CharacterSheetLinkList;
use sheetlist::CharacterSheetList;
use weakcomponentlink::WeakComponentLink;

pub struct Model {
    agent_sock: Option<WebSocketTask>,
    dir_path_element: NodeRef,
    entries_element: NodeRef,
    link: ComponentLink<Self>,
    links_list_element: WeakComponentLink<CharacterSheetLinkList>,
    sheets_list_element: WeakComponentLink<CharacterSheetList>,
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
            links_list_element:
                WeakComponentLink::<CharacterSheetLinkList>::default(),
            sheets_list_element:
                WeakComponentLink::<CharacterSheetList>::default(),
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
                ConsoleService::debug(&format!("Socket message {:?}", &m));
                match m {
                    GCSAgentMessage::RequestChDirResult(result) => {
                        if let Err(text) = result {
                            ConsoleService::error(&text);
                        }
                    }
                    GCSAgentMessage::RequestSheetContentsResult(result) => {
                        let (path, contents) = match result {
                            Err(text) => {
                                ConsoleService::error(&text);
                                return false;
                            }
                            Ok(GCSFile {
                                path,
                                file: contents,
                            }) => (path, contents),
                        };
                        let links_list =
                            self.links_list_element.borrow().clone().unwrap();
                        let sheets_list =
                            self.sheets_list_element.borrow().clone().unwrap();
                        let character = match contents {
                            gcs::FileKind::Character(
                                gcs::character::Character::V1(character),
                            ) => character,
                            _ => {
                                ConsoleService::error("File not V1 character");
                                return false;
                            }
                        };
                        links_list.send_message(
                            <CharacterSheetLinkList as Component>::Message::SheetAdded(character.profile.name.clone()));
                        sheets_list.send_message(
                            <CharacterSheetList as Component>::Message::SheetAdded(path, character));
                    }
                    GCSAgentMessage::RequestWatchResult(result) => {
                        if let Err(text) = result {
                            ConsoleService::error(&text);
                        }
                    }
                    GCSAgentMessage::FileChangeNotification(path) => {
                        ConsoleService::log(&path);
                        let ws = self.agent_sock.as_mut().unwrap();
                        match bincode::serialize(
                            &WebUIMessage::RequestSheetContents(path),
                        ) {
                            Ok(bytes) => ws.send_binary(Ok(bytes)),
                            Err(e) => {
                                ConsoleService::error(&format!("{:?}", e))
                            }
                        }
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
                                                    name,
                                                    &serde_json::to_string(
                                                        &entry).unwrap(),
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
                                                    &text,
                                                    &serde_json::to_string(
                                                        &entry).unwrap(),
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
                let msgs = match entry {
                    FileEntry::Directory(path) => {
                        vec![WebUIMessage::RequestChDir(path)]
                    }
                    FileEntry::GCSFile(path) => {
                        vec![
                            WebUIMessage::RequestWatch(path.clone()),
                            WebUIMessage::RequestSheetContents(path),
                        ]
                    }
                };
                for msg in msgs {
                    match bincode::serialize(&msg) {
                        Ok(bytes) => ws.send_binary(Ok(bytes)),
                        Err(e) => ConsoleService::error(&format!("{:?}", e)),
                    }
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
        let sheets: Vec<(String, gcs::character::CharacterV1)> = vec![];
        html! {
          <>
            <div id="nav">
              <h1>{"Navigation"}</h1>
              <ul>
                // TODO: There must be something to automate a sitemap
                <li><a href="#nav">{"Navigations"}</a></li>
                <li>
                  <a href="#sheets">{"Character Sheets"}</a>
                  <ul id="nav-sheets">
                  <CharacterSheetLinkList
                   link_prefix="sheets-"
                   names=sheets.iter().map(|(name, _)| {
                       name.clone()
                   }).collect::<Vec<String>>()
                   weak_link=&self.links_list_element
                  />
                  </ul>
                </li>
                <li><a href="#file-browser">{"File Browser"}</a></li>
              </ul>
            </div>
            <div id="sheets">
              <h1>{"Character Sheets"}</h1>
              <CharacterSheetList
               character_sheets=sheets,
               link_prefix="sheets-"
               weak_link=&self.sheets_list_element/>
            </div>
            <div id="file-browser">
              <h1>{"File Browser"}</h1>
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
                           ConsoleService::info(
                               &format!("{:?}", select_element.value()));
                           let entry = serde_json::from_str(
                               &select_element.value()).unwrap();
                           Msg::DirectoryEntrySelected(entry)
                       }
                       _ => Msg::Ignore
                     })>
                    </select>
                  </td>
                </tr>
              </table>
            </div>
          </>
        }
    }
}

#[wasm_bindgen(start)]
pub fn main() -> Result<(), JsValue> {
    yew::start_app::<Model>();
    Ok(())
}
