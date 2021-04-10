#![allow(clippy::single_component_path_imports)]

use std::collections::{BTreeMap, HashMap};

use web_sys::{FocusEvent, HtmlInputElement};
use yew::{
    self, html, Component, ComponentLink, Html, NodeRef, Properties,
    ShouldRender,
};

use gmtool_common::PortableOsString;

use crate::weakcomponentlink::WeakComponentLink;

#[derive(Clone, PartialEq, Properties)]
pub struct Props {
    #[prop_or_default]
    pub character_sheets:
        BTreeMap<PortableOsString, gcs::character::CharacterV1>,
    #[prop_or_default]
    pub link_prefix: String,
    #[prop_or_default]
    pub weak_link: WeakComponentLink<CharacterSheetList>,
}

pub struct CharacterSheetList {
    inputs: HashMap<PortableOsString, NodeRef>,
    props: Props,
}

pub enum Msg {
    SheetAdded(PortableOsString, gcs::character::CharacterV1),
    SheetModified(PortableOsString),
}

impl Component for CharacterSheetList {
    type Message = Msg;
    type Properties = Props;

    fn create(props: Self::Properties, link: ComponentLink<Self>) -> Self {
        let inputs = HashMap::new();
        *props.weak_link.borrow_mut() = Some(link);
        Self { inputs, props }
    }

    fn change(&mut self, props: Self::Properties) -> ShouldRender {
        if self.props != props {
            self.props = props;
            true
        } else {
            false
        }
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        match msg {
            Msg::SheetAdded(path, character) => {
                self.inputs.insert(path.clone(), NodeRef::default());
                self.props.character_sheets.insert(path, character);
                true
            }
            Msg::SheetModified(path) => {
                let hp_input = self
                    .inputs
                    .get(&path)
                    .expect("Sheet not deleted between click and msg")
                    .cast::<HtmlInputElement>()
                    .unwrap();
                let hp = hp_input.value().parse().expect("HP text parseable");
                let sheet = self
                    .props
                    .character_sheets
                    .get_mut(&path)
                    .expect("Sheet not deleted between click and msg");
                sheet.set_hit_points(hp);

                false
            }
        }
    }

    fn view(&self) -> Html {
        let link_ref = self.props.weak_link.borrow_mut();
        let link = link_ref.as_ref().unwrap();
        html! {
          {
            for self.props.character_sheets.iter().map(|(path, character)| {
              let (hp, maxHP) = character.get_hit_points();
              let form_cb_path = path.clone();
              let form_cb = link.callback(move |evt: FocusEvent| {
                  evt.prevent_default();
                  Msg::SheetModified(form_cb_path.clone())
              });
              let hp_input = self
                  .inputs
                  .get(path)
                  .expect("Change message created ref before view")
                  .clone();
              html! {
                <div id=format!("{}{}",
                                self.props.link_prefix,
                                path.to_str_lossy())>
                  <h2>{character.profile.name.clone()}</h2>
                  <form onsubmit=form_cb>
                    <input type="number" value=hp ref=hp_input/>{"/"}{maxHP}
                  </form>
                </div>
              }
            })
          }
        }
    }
}
