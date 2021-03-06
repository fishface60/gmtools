#![allow(clippy::single_component_path_imports, clippy::large_enum_variant)]
#![recursion_limit = "1024"]

mod navlist;
mod sheetlist;
mod weakcomponentlink;

use std::{collections::BTreeMap, convert::TryInto};

use anyhow::anyhow;
use log::{debug, error, info, warn};
use url::Url;
use wasm_bindgen::prelude::*;
use web_sys::{
    FocusEvent, HtmlInputElement, HtmlOptionElement, HtmlSelectElement,
};
use yew::{
    self,
    format::{Json, Nothing},
    html,
    services::{
        fetch::{FetchTask, Method, Request, Response},
        FetchService, Task,
    },
    ChangeData, Component, ComponentLink, Html, NodeRef, ShouldRender,
};
use yew_event_source::{
    EventSourceService, EventSourceStatus, EventSourceTask,
};

use gmtool_common::{FileEntry, PortableOsString, ReadResponse, WriteRequest};
use navlist::CharacterSheetLinkList;
use sheetlist::CharacterSheetList;
use weakcomponentlink::WeakComponentLink;

pub struct Model {
    agentaddr: Option<Url>,
    curdir: PortableOsString,
    dir_path_element: NodeRef,
    entries_element: NodeRef,
    fetch_tasks: Vec<FetchTask>,
    link: WeakComponentLink<Self>,
    links_list_element: WeakComponentLink<CharacterSheetLinkList>,
    sheets_list_element: WeakComponentLink<CharacterSheetList>,
    sse_con: Option<EventSourceTask>,
    secret: Option<u32>,
}

impl Model {
    fn clear_fetch_tasks(&mut self) {
        self.fetch_tasks.retain(FetchTask::is_active);
    }

    fn build_request<T>(
        &self,
        method: Method,
        path: &str,
        body: T,
    ) -> Result<Request<T>, anyhow::Error> {
        let mut uri = match self.agentaddr {
            Some(ref agentaddr) => agentaddr.clone(),
            None => anyhow::bail!("No agent address"),
        };
        uri.set_path(path);
        Ok(Request::builder()
            .uri(uri.as_str())
            .method(method)
            .body(body)?)
    }

    fn request_auth(&mut self) -> Result<(), anyhow::Error> {
        let link = self.link.borrow().clone().unwrap();
        let secret = self
            .secret
            .ok_or_else(|| anyhow!("Can't auth without secret"))?;
        let req = self.build_request(
            Method::POST,
            "/auth",
            Ok(bincode::serialize(&secret)?),
        )?;
        let clos = |response: Response<Nothing>| {
            if !response.status().is_success() {
                warn!("Auth returned {}", response.status());
                return Msg::Ignore;
            }
            Msg::Authenticated
        };
        let task = FetchService::fetch_binary(req, link.callback(clos))?;
        self.fetch_tasks.push(task);
        Ok(())
    }

    fn connect_sse(&mut self) -> Result<(), anyhow::Error> {
        let link = self.link.borrow().clone().unwrap();
        let mut uri = match self.agentaddr {
            Some(ref agentaddr) => agentaddr.clone(),
            None => anyhow::bail!("No agent address"),
        };
        uri.set_path("/sse");

        let mut sse_con = EventSourceService::new()
            .connect(
                uri.as_str(),
                link.callback(|status| {
                    if status == EventSourceStatus::Error {
                        error!("event source error");
                    }
                    Msg::Ignore
                }),
            )
            .map_err(|s| anyhow!("SSE Connect failed: {}", s))?;
        sse_con.add_event_listener(
            "file_change",
            link.callback(|Json(data)| match data {
                Ok(path) => Msg::FileChange(path),
                Err(e) => {
                    error!("{:?}", e);
                    Msg::Ignore
                }
            }),
        );
        self.sse_con = Some(sse_con);
        Ok(())
    }

    fn request_chdir(
        &mut self,
        path: Option<&PortableOsString>,
    ) -> Result<(), anyhow::Error> {
        let link = self.link.borrow().clone().unwrap();
        let req = self.build_request(
            Method::POST,
            "/chdir",
            Ok(bincode::serialize(path.unwrap_or(&self.curdir))?),
        )?;
        let clos = |response: Response<Result<Vec<u8>, anyhow::Error>>| {
            let bytes = match response.into_body() {
                Ok(bytes) => bytes,
                Err(e) => {
                    error!("{:?}", e);
                    return Msg::Ignore;
                }
            };
            let contents = match bincode::deserialize(&bytes) {
                Ok(contents) => contents,
                Err(e) => {
                    error!("{:?}", e);
                    return Msg::Ignore;
                }
            };
            Msg::RequestChDirResponse(contents)
        };
        let task = FetchService::fetch_binary(req, link.callback(clos))?;
        self.fetch_tasks.push(task);
        Ok(())
    }

    fn request_lsdir(&mut self) -> Result<(), anyhow::Error> {
        let link = self.link.borrow().clone().unwrap();
        let req = self.build_request(Method::GET, "/lsdir", Nothing)?;
        let clos = |response: Response<Result<Vec<u8>, anyhow::Error>>| {
            let bytes = match response.into_body() {
                Ok(bytes) => bytes,
                Err(e) => {
                    error!("{:?}", e);
                    return Msg::Ignore;
                }
            };
            let contents = match bincode::deserialize(&bytes) {
                Ok(contents) => contents,
                Err(e) => {
                    error!("{:?}", e);
                    return Msg::Ignore;
                }
            };
            Msg::RequestLsDirResponse(contents)
        };
        let task = FetchService::fetch_binary(req, link.callback(clos))?;
        self.fetch_tasks.push(task);
        Ok(())
    }

    fn request_read(
        &mut self,
        path: PortableOsString,
    ) -> Result<(), anyhow::Error> {
        let link = self.link.borrow().clone().unwrap();
        let req = self.build_request(
            Method::POST,
            "/read",
            Ok(bincode::serialize(&path)?),
        )?;
        let clos = move |response: Response<Result<Vec<u8>, anyhow::Error>>| {
            let bytes = match response.into_body() {
                Ok(bytes) => bytes,
                Err(e) => {
                    error!("Response into body: {:?}", e);
                    return Msg::Ignore;
                }
            };
            let (path, contents) = match bincode::deserialize(&bytes) {
                Ok(ReadResponse { path, contents }) => (path, contents),
                Err(e) => {
                    error!("Body deserialize: {:?}", e);
                    return Msg::Ignore;
                }
            };
            Msg::RequestSheetContentsResponse(path, contents)
        };
        let task = FetchService::fetch_binary(req, link.callback_once(clos))?;
        self.fetch_tasks.push(task);
        Ok(())
    }

    fn request_watch(
        &mut self,
        path: &PortableOsString,
    ) -> Result<(), anyhow::Error> {
        let link = self.link.borrow().clone().unwrap();
        let req = self.build_request(
            Method::POST,
            "/watch",
            Ok(bincode::serialize(path)?),
        )?;
        let clos = move |response: Response<Result<Vec<u8>, anyhow::Error>>| {
            debug!("watch response: {:?}", &response);
            if let Err(e) = response.into_body() {
                error!("{:?}", e);
            }
            Msg::Ignore
        };
        let task = FetchService::fetch_binary(req, link.callback(clos))?;
        self.fetch_tasks.push(task);
        Ok(())
    }

    fn request_write(
        &mut self,
        path: PortableOsString,
        contents: gcs::FileKind,
    ) -> Result<(), anyhow::Error> {
        let link = self.link.borrow().clone().unwrap();
        let req_body = WriteRequest { path, contents };
        let req = self.build_request(
            Method::POST,
            "/write",
            Ok(bincode::serialize(&req_body)?),
        )?;
        let clos = move |response: Response<Result<Vec<u8>, anyhow::Error>>| {
            let bytes = match response.into_body() {
                Ok(bytes) => bytes,
                Err(e) => {
                    error!("Response into body: {:?}", e);
                    return Msg::Ignore;
                }
            };
            let path: PortableOsString = match bincode::deserialize(&bytes) {
                Ok(path) => path,
                Err(e) => {
                    error!("Body deserialize: {:?}", e);
                    return Msg::Ignore;
                }
            };
            if path != req_body.path {
                warn!(
                    "Save requested path {:?} but resolved to {:?}",
                    &path, &req_body.path
                );
            }
            Msg::Ignore
        };
        let task = FetchService::fetch_binary(req, link.callback_once(clos))?;
        self.fetch_tasks.push(task);
        Ok(())
    }
}

#[derive(Debug)]
pub enum Msg {
    Authenticated,
    DirectoryEntrySelected(FileEntry),
    DirectoryPathSubmitted,
    FileChange(PortableOsString),
    RequestSheetContentsResponse(PortableOsString, gcs::FileKind),
    RequestChDirResponse(PortableOsString),
    RequestLsDirResponse(Vec<FileEntry>),
    SheetSubmit(PortableOsString, gcs::character::Character),
    Ignore,
}

fn parse_params() -> (Option<Url>, Option<u32>) {
    let window = match web_sys::window() {
        None => {
            error!("no window object exists");
            return (None, None);
        }
        Some(window) => window,
    };
    let href = match window.location().href() {
        Err(e) => {
            error!("window missing url {:?}", e);
            return (None, None);
        }
        Ok(href) => href,
    };
    let url = match Url::parse(&href) {
        Err(e) => {
            error!("window url parse failed {:?}", e);
            return (None, None);
        }
        Ok(url) => url,
    };
    let mut agentaddr = None;
    let mut secret = None;
    for (k, v) in url.query_pairs() {
        if k == "agentaddr" {
            agentaddr = Some(v);
            if secret.is_some() {
                break;
            }
        } else if k == "token" {
            let array = match base64::decode(v.into_owned()) {
                Ok(vec) => vec.try_into().expect("Token 4 bytes"),
                Err(e) => {
                    error!("token decode failed {:?}", e);
                    continue;
                }
            };
            secret = Some(u32::from_le_bytes(array));
            if agentaddr.is_some() {
                break;
            }
        }
    }

    match window.history() {
        Err(e) => {
            warn!("could not access history: {:?}", e);
        }
        Ok(hst) => {
            if let Err(e) =
                hst.replace_state_with_url(&JsValue::NULL, "", Some("/webui"))
            {
                warn!("could not change url: {:?}", e);
            }
        }
    }

    let agentaddr = match agentaddr {
        Some(agentaddr) => agentaddr.to_string(),
        None => {
            info!("url did not include agentaddr");
            href
        }
    };

    let agentaddr = match Url::parse(&agentaddr) {
        Err(e) => {
            error!("agentaddr parse failed {:?}", e);
            return (None, None);
        }
        Ok(agentaddr) => Some(agentaddr),
    };

    (agentaddr, secret)
}

impl Component for Model {
    type Message = Msg;
    type Properties = ();

    fn create(_: Self::Properties, link: ComponentLink<Self>) -> Self {
        let (agentaddr, secret) = parse_params();

        let curdir = PortableOsString::from(".");
        let mut model = Model {
            agentaddr,
            curdir,
            dir_path_element: NodeRef::default(),
            entries_element: NodeRef::default(),
            fetch_tasks: vec![],
            link: WeakComponentLink::new(link),
            links_list_element:
                WeakComponentLink::<CharacterSheetLinkList>::default(),
            secret,
            sheets_list_element:
                WeakComponentLink::<CharacterSheetList>::default(),
            sse_con: None,
        };

        if let Err(e) = model.request_auth() {
            error!("Auth failed: {:?}", e);
        }

        model
    }

    fn change(&mut self, _: Self::Properties) -> bool {
        false
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        match msg {
            Msg::Authenticated => {
                let should_reconnect_sse = match self.sse_con {
                    Some(ref sse_con) => !sse_con.is_active(),
                    None => true,
                };
                if should_reconnect_sse {
                    if let Err(e) = self.connect_sse() {
                        error!("Connect sse failed {:?}", e);
                    }
                }

                if let Err(e) = self.request_chdir(None) {
                    error!("Request chdir failed {:?}", e);
                }

                false
            }
            Msg::DirectoryEntrySelected(entry) => {
                match entry {
                    FileEntry::Directory(ref path) => {
                        if let Err(e) = self.request_chdir(Some(path)) {
                            error!("Request chdir failed {:?}", e);
                        };
                    }
                    FileEntry::GCSFile(path) => {
                        if let Err(e) = self.request_watch(&path) {
                            error!("Request watch failed {:?}", e);
                        };
                        if let Err(e) = self.request_read(path) {
                            error!("Request read failed {:?}", e);
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
                if let Err(e) = self.request_chdir(Some(&path)) {
                    error!("Request chdir failed {:?}", e);
                };
                false
            }
            Msg::FileChange(path) => {
                if let Err(e) = self.request_read(path) {
                    error!("Request read failed {:?}", e);
                };
                false
            }
            Msg::RequestChDirResponse(path) => {
                self.clear_fetch_tasks();
                self.dir_path_element
                    .cast::<HtmlInputElement>()
                    .expect("dir_path instantiated")
                    .set_value(&path.to_str_lossy());
                self.curdir = path;
                if let Err(e) = self.request_lsdir() {
                    error!("Request lsdir failed {:?}", e);
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
                            debug!("File {:?}", name);
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
                            debug!("Directory {:?}", name);
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
                        error!("File not V1 character");
                        return false;
                    }
                };
                links_list.send_message(
                    <CharacterSheetLinkList as Component>::Message::SheetAdded(
                        path.clone(),
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
            Msg::SheetSubmit(path, character) => {
                let contents = gcs::FileKind::Character(character);
                if let Err(e) = self.request_write(path, contents) {
                    error!("Request write failed {:?}", e);
                };
                false
            }
            Msg::Ignore => false,
        }
    }

    fn view(&self) -> Html {
        let link = self.link.borrow().clone().unwrap();
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
                   names=BTreeMap::new()
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
               character_sheets=BTreeMap::new()
               link_prefix="sheets-"
               model_link=&self.link
               weak_link=&self.sheets_list_element/>
            </div>
            <div id="file-browser">
              <h1>{"File Browser"}</h1>
              <table>
                <tr>
                  <th>{"Directory"}</th>
                  <td>
                    <form
                     onsubmit=link.callback(|evt: FocusEvent| {
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
                     onchange=link.callback(|evt: ChangeData| match evt {
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
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    wasm_logger::init(wasm_logger::Config::default());
    yew::start_app::<Model>();
    Ok(())
}
