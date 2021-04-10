use std::cell::RefCell;
use std::ops::Deref;
use std::rc::Rc;
use yew::html::{Component, ComponentLink};

pub struct WeakComponentLink<COMP: Component>(
    Rc<RefCell<Option<ComponentLink<COMP>>>>,
);

impl<COMP: Component> WeakComponentLink<COMP> {
    pub fn new(link: ComponentLink<COMP>) -> Self {
        Self(Rc::new(RefCell::new(Some(link))))
    }
}

impl<COMP: Component> Default for WeakComponentLink<COMP> {
    fn default() -> Self {
        Self(Rc::new(RefCell::new(None)))
    }
}

impl<COMP: Component> Deref for WeakComponentLink<COMP> {
    type Target = Rc<RefCell<Option<ComponentLink<COMP>>>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<COMP: Component> Clone for WeakComponentLink<COMP> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<COMP: Component> PartialEq for WeakComponentLink<COMP> {
    fn eq(&self, other: &WeakComponentLink<COMP>) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}
