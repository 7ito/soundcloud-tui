#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub next_href: Option<String>,
}

impl<T> Page<T> {
    pub fn map<U>(self, mut map: impl FnMut(T) -> U) -> Page<U> {
        Page {
            items: self.items.into_iter().map(&mut map).collect(),
            next_href: self.next_href,
        }
    }
}
