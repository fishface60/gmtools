#![allow(clippy::single_component_path_imports, clippy::large_enum_variant)]
#![recursion_limit = "1024"]

mod navlist;
mod sheetlist;
mod weakcomponentlink;

use url::Url;
use wasm_bindgen::prelude::*;
use web_sys::{
    FocusEvent, HtmlInputElement, HtmlOptionElement, HtmlSelectElement,
};
use yew::{
    self,
    format::Json,
    html,
    services::{
        fetch::{FetchTask, Method, Request, Response},
        ConsoleService, FetchService, Task,
    },
    ChangeData, Component, ComponentLink, Html, NodeRef, ShouldRender,
};
use yew_event_source::{
    EventSourceService, EventSourceStatus, EventSourceTask,
};

use gmtool_common::{FileEntry, PortableOsString};
use navlist::CharacterSheetLinkList;
use sheetlist::CharacterSheetList;
use weakcomponentlink::WeakComponentLink;

pub struct Model {
    agentaddr: Option<Url>,
    dir_path_element: NodeRef,
    entries_element: NodeRef,
    fetch_tasks: Vec<FetchTask>,
    link: ComponentLink<Self>,
    links_list_element: WeakComponentLink<CharacterSheetLinkList>,
    sheets_list_element: WeakComponentLink<CharacterSheetList>,
    _sse_con: Option<EventSourceTask>,
}

impl Model {
    fn clear_fetch_tasks(&mut self) {
        self.fetch_tasks.retain(FetchTask::is_active);
    }
}

#[derive(Debug)]
pub enum Msg {
    DirectoryEntrySelected(FileEntry),
    DirectoryPathSubmitted,
    FileChange(PortableOsString),
    RequestSheetContentsResponse(PortableOsString, gcs::FileKind),
    RequestChDirResponse(PortableOsString),
    RequestLsDirResponse(Vec<FileEntry>),
    Ignore,
}

fn connect_sse(
    link: &mut ComponentLink<Model>,
) -> (Option<Url>, Option<EventSourceTask>) {
    let window = match web_sys::window() {
        None => {
            ConsoleService::error("window object exists");
            return (None, None);
        }
        Some(window) => window,
    };
    let href = match window.location().href() {
        Err(e) => {
            ConsoleService::error(&format!("window missing url {:?}", e));
            return (None, None);
        }
        Ok(href) => href,
    };
    let url = match Url::parse(&href) {
        Err(e) => {
            ConsoleService::error(&format!("window url parse failed {:?}", e));
            return (None, None);
        }
        Ok(url) => url,
    };
    let mut agentaddr = None;
    for (k, v) in url.query_pairs() {
        if k == "agentaddr" {
            agentaddr = Some(v);
            break;
        }
    }
    let agentaddr = match agentaddr {
        Some(agentaddr) => agentaddr.to_string(),
        None => {
            ConsoleService::info("url did not include agentaddr");
            href
        }
    };

    let mut agentaddr = match Url::parse(&agentaddr) {
        Err(e) => {
            ConsoleService::error(&format!("agentaddr parse failed {:?}", e));
            return (None, None);
        }
        Ok(agentaddr) => agentaddr,
    };
    agentaddr.set_path("/sse");

    let mut sse_con = match EventSourceService::new().connect(
        agentaddr.as_str(),
        link.callback(|status| {
            if status == EventSourceStatus::Error {
                ConsoleService::error("event source error");
            }
            Msg::Ignore
        }),
    ) {
        Ok(sse_con) => sse_con,
        Err(e) => {
            ConsoleService::error(&format!("sse connect failed {:?}", e));
            return (Some(agentaddr), None);
        }
    };
    sse_con.add_event_listener(
        "file_change",
        link.callback(|Json(data)| match data {
            Ok(path) => Msg::FileChange(path),
            Err(e) => {
                ConsoleService::error(&format!("{:?}", e));
                Msg::Ignore
            }
        }),
    );
    (Some(agentaddr), Some(sse_con))
}

fn request_chdir(
    agentaddr: &Url,
    link: &mut ComponentLink<Model>,
    path: PortableOsString,
) -> Result<FetchTask, anyhow::Error> {
    let mut uri = agentaddr.clone();
    uri.set_path("/chdir");
    let req = Request::builder()
        .uri(uri.as_str())
        .method(Method::POST)
        .body(Ok(bincode::serialize(&path)?))?;
    let task = FetchService::fetch_binary(
        req,
        link.callback(|response: Response<Result<Vec<u8>, anyhow::Error>>| {
            let bytes = match response.into_body() {
                Ok(bytes) => bytes,
                Err(e) => {
                    ConsoleService::error(&format!("{:?}", e));
                    return Msg::Ignore;
                }
            };
            let contents = match bincode::deserialize(&bytes) {
                Ok(contents) => contents,
                Err(e) => {
                    ConsoleService::error(&format!("{:?}", e));
                    return Msg::Ignore;
                }
            };
            Msg::RequestChDirResponse(contents)
        }),
    )?;
    Ok(task)
}

fn request_lsdir(
    agentaddr: &Url,
    link: &mut ComponentLink<Model>,
    path: PortableOsString,
) -> Result<FetchTask, anyhow::Error> {
    ConsoleService::log(&format!("Path: {:?}", &path));
    let mut uri = agentaddr.clone();
    uri.set_path("/lsdir");
    ConsoleService::log(&format!("URI: {:?}", &uri));
    let bytes = bincode::serialize(&path)?;
    ConsoleService::log(&format!("Bytes: {:?}", &bytes));
    let req = Request::builder()
        .uri(uri.as_str())
        .method(Method::POST)
        .body(Ok(bytes))?;
    ConsoleService::log(&format!("Request: {:?}", &req));

    let task = FetchService::fetch_binary(
        req,
        link.callback(|response: Response<Result<Vec<u8>, anyhow::Error>>| {
            let bytes = match response.into_body() {
                Ok(bytes) => bytes,
                Err(e) => {
                    ConsoleService::error(&format!("{:?}", e));
                    return Msg::Ignore;
                }
            };
            let contents = match bincode::deserialize(&bytes) {
                Ok(contents) => contents,
                Err(e) => {
                    ConsoleService::error(&format!("{:?}", e));
                    return Msg::Ignore;
                }
            };
            Msg::RequestLsDirResponse(contents)
        }),
    )?;
    Ok(task)
}

fn request_read(
    agentaddr: &Url,
    link: &mut ComponentLink<Model>,
    path: PortableOsString,
) -> Result<FetchTask, anyhow::Error> {
    let mut uri = agentaddr.clone();
    uri.set_path("/read");
    let req = Request::builder()
        .uri(uri.as_str())
        .method(Method::POST)
        .body(Ok(bincode::serialize(&path)?))?;
    let path = path.clone();
    let clos = move |response: Response<Result<Vec<u8>, anyhow::Error>>| {
        let bytes = match response.into_body() {
            Ok(bytes) => bytes,
            Err(e) => {
                ConsoleService::error(&format!("Response into body: {:?}", e));
                return Msg::Ignore;
            }
        };
        let contents = match serde_cbor::from_slice(&bytes) {
            Ok(contents) => contents,
            Err(e) => {
                ConsoleService::error(&format!("Body deserialize: {:?}", e));
                return Msg::Ignore;
            }
        };
        Msg::RequestSheetContentsResponse(path, contents)
    };
    let task = FetchService::fetch_binary(req, link.callback_once(clos))?;
    Ok(task)
}

fn request_watch(
    agentaddr: &Url,
    link: &mut ComponentLink<Model>,
    path: PortableOsString,
) -> Result<FetchTask, anyhow::Error> {
    let mut uri = agentaddr.clone();
    uri.set_path("/watch");
    let req = Request::builder()
        .uri(uri.as_str())
        .method(Method::POST)
        .body(Ok(bincode::serialize(&path)?))?;
    let clos = move |response: Response<Result<Vec<u8>, anyhow::Error>>| {
        ConsoleService::debug(&format!("watch response: {:?}", &response));
        if let Err(e) = response.into_body() {
            ConsoleService::error(&format!("{:?}", e));
        }
        Msg::Ignore
    };
    let task = FetchService::fetch_binary(req, link.callback_once(clos))?;
    Ok(task)
}

impl Component for Model {
    type Message = Msg;
    type Properties = ();

    fn create(_: Self::Properties, mut link: ComponentLink<Self>) -> Self {
        let (agentaddr, _sse_con) = connect_sse(&mut link);

        let mut fetch_tasks = vec![];
        match agentaddr {
            Some(ref agentaddr) => {
                match request_chdir(
                    &agentaddr,
                    &mut link,
                    PortableOsString::from("."),
                ) {
                    Ok(fetch_task) => fetch_tasks.push(fetch_task),
                    Err(e) => {
                        ConsoleService::error(&format!(
                            "Request chdir failed {:?}",
                            e
                        ));
                    }
                }
            }
            None => (),
        };

        Model {
            agentaddr,
            dir_path_element: NodeRef::default(),
            entries_element: NodeRef::default(),
            fetch_tasks,
            link,
            links_list_element:
                WeakComponentLink::<CharacterSheetLinkList>::default(),
            sheets_list_element:
                WeakComponentLink::<CharacterSheetList>::default(),
            _sse_con,
        }
    }

    fn change(&mut self, _: Self::Properties) -> bool {
        false
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        match msg {
            Msg::DirectoryEntrySelected(entry) => {
                match entry {
                    FileEntry::Directory(path) => {
                        match request_chdir(
                            self.agentaddr.as_ref().unwrap(),
                            &mut self.link,
                            path,
                        ) {
                            Ok(fetch_task) => self.fetch_tasks.push(fetch_task),
                            Err(e) => {
                                ConsoleService::error(&format!(
                                    "Request chdir failed {:?}",
                                    e
                                ));
                            }
                        };
                    }
                    FileEntry::GCSFile(path) => {
                        match request_watch(
                            self.agentaddr.as_ref().unwrap(),
                            &mut self.link,
                            path.clone(),
                        ) {
                            Ok(fetch_task) => self.fetch_tasks.push(fetch_task),
                            Err(e) => {
                                ConsoleService::error(&format!(
                                    "Request watch failed {:?}",
                                    e
                                ));
                            }
                        };
                        match request_read(
                            self.agentaddr.as_ref().unwrap(),
                            &mut self.link,
                            path,
                        ) {
                            Ok(fetch_task) => self.fetch_tasks.push(fetch_task),
                            Err(e) => {
                                ConsoleService::error(&format!(
                                    "Request read failed {:?}",
                                    e
                                ));
                            }
                        };
                    }
                };
                false
            }
            Msg::DirectoryPathSubmitted => {
                let path: PortableOsString = self
                    .dir_path_element
                    .cast::<HtmlInputElement>()
                    .unwrap()
                    .value()
                    .into();
                match request_chdir(
                    self.agentaddr.as_ref().unwrap(),
                    &mut self.link,
                    path,
                ) {
                    Ok(fetch_task) => self.fetch_tasks.push(fetch_task),
                    Err(e) => {
                        ConsoleService::error(&format!(
                            "Request chdir failed {:?}",
                            e
                        ));
                    }
                };
                false
            }
            Msg::FileChange(path) => {
                match request_read(
                    self.agentaddr.as_ref().unwrap(),
                    &mut self.link,
                    path,
                ) {
                    Ok(fetch_task) => self.fetch_tasks.push(fetch_task),
                    Err(e) => {
                        ConsoleService::error(&format!(
                            "Request read failed {:?}",
                            e
                        ));
                    }
                };
                false
            }
            Msg::RequestChDirResponse(path) => {
                self.clear_fetch_tasks();
                self.dir_path_element
                    .cast::<HtmlInputElement>()
                    .expect("dir_path instantiated")
                    .set_value(&path.to_str_lossy());
                match request_lsdir(
                    self.agentaddr.as_ref().unwrap(),
                    &mut self.link,
                    path,
                ) {
                    Ok(fetch_task) => self.fetch_tasks.push(fetch_task),
                    Err(e) => {
                        ConsoleService::error(&format!(
                            "Request lsdir failed {:?}",
                            e
                        ));
                    }
                };

                false
            }
            Msg::RequestLsDirResponse(entries) => {
                self.clear_fetch_tasks();
                let entries_element = self
                    .entries_element
                    .cast::<HtmlSelectElement>()
                    .expect("entries select intstantiated");
                for _ in 0..entries_element.length() {
                    entries_element.remove_with_index(0);
                }

                let mut text = String::from("../");
                let option = HtmlOptionElement::new_with_text_and_value(
                    &text,
                    &serde_json::to_string(&FileEntry::Directory(
                        PortableOsString::from(".."),
                    ))
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
                                    name.to_str_lossy(),
                                    &serde_json::to_string(&entry).unwrap(),
                                )
                                .unwrap();
                            entries_element
                                .add_with_html_option_element(&option)
                                .unwrap();
                            ConsoleService::log(&format!("File {:?}", name));
                        }
                        FileEntry::Directory(ref name) => {
                            text.clear();
                            text.push_str(name.to_str_lossy());
                            text.push('/');
                            let option =
                                HtmlOptionElement::new_with_text_and_value(
                                    &text,
                                    &serde_json::to_string(&entry).unwrap(),
                                )
                                .unwrap();
                            entries_element
                                .add_with_html_option_element(&option)
                                .unwrap();
                            ConsoleService::log(&format!(
                                "Directory {:?}",
                                name
                            ));
                        }
                    }
                }
                false
            }
            Msg::RequestSheetContentsResponse(path, contents) => {
                self.clear_fetch_tasks();
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
                    <CharacterSheetLinkList as Component>::Message::SheetAdded(
                        character.profile.name.clone(),
                    ),
                );
                sheets_list.send_message(
                    <CharacterSheetList as Component>::Message::SheetAdded(
                        path, character,
                    ),
                );
                false
            }
            Msg::Ignore => false,
        }
    }

    fn view(&self) -> Html {
        let sheets: Vec<(PortableOsString, gcs::character::CharacterV1)> =
            vec![];
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
                   names=sheets.iter().map(|(_, sheet)| {
                       sheet.profile.name.clone()
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
                           let entry = serde_json::from_str(
                               &select_element.value()).expect("select value");
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
