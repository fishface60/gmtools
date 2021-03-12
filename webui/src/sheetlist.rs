#![allow(clippy::single_component_path_imports)]

use yew::{
    self, html, Component, ComponentLink, Html, Properties, ShouldRender,
};

use crate::weakcomponentlink::WeakComponentLink;

#[derive(Clone, PartialEq, Properties)]
pub struct Props {
    #[prop_or_default]
    pub character_sheets: Vec<(String, gcs::character::CharacterV1)>,
    #[prop_or_default]
    pub link_prefix: String,
    #[prop_or_default]
    pub weak_link: WeakComponentLink<CharacterSheetList>,
}

pub struct CharacterSheetList {
    props: Props,
}

pub enum Msg {
    SheetAdded(String, gcs::character::CharacterV1),
}

impl Component for CharacterSheetList {
    type Message = Msg;
    type Properties = Props;

    fn create(props: Self::Properties, link: ComponentLink<Self>) -> Self {
        *props.weak_link.borrow_mut() = Some(link);
        Self { props }
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
                self.props.character_sheets.push((path, character));
                true
            }
        }
    }

    fn view(&self) -> Html {
        html! {
          {
            for self.props.character_sheets.iter().map(|(path, character)| {
              let (hp, maxHP) = character.get_hit_points();
              html! {
                <div id=format!("{}{}", self.props.link_prefix, character.profile.name)>
                  <h2>{character.profile.name.clone()}</h2>
                  <input type="number" value=hp/>{"/"}{maxHP}
                </div>
              }
            })
          }
        }
    }
}
