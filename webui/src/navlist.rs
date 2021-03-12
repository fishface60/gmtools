#![allow(clippy::single_component_path_imports)]

use yew::{
    self, html, Component, ComponentLink, Html, Properties, ShouldRender,
};

use crate::weakcomponentlink::WeakComponentLink;

#[derive(Clone, PartialEq, Properties)]
pub struct Props {
    #[prop_or_default]
    pub names: Vec<String>,
    #[prop_or_default]
    pub link_prefix: String,
    #[prop_or_default]
    pub weak_link: WeakComponentLink<CharacterSheetLinkList>,
}

pub struct CharacterSheetLinkList {
    props: Props,
}

pub enum Msg {
    SheetAdded(String),
}

impl Component for CharacterSheetLinkList {
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
            Msg::SheetAdded(name) => {
                self.props.names.push(name);
                true
            }
        }
    }

    fn view(&self) -> Html {
        html! {
          {
            for self.props.names.iter().map(|name| {
              html! {
                <li>
                  <a href=format!("#{}{}", self.props.link_prefix, name)>
                  {name}
                  </a>
                </li>
              }
            })
          }
        }
    }
}
