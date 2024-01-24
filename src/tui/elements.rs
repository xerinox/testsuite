use async_trait::async_trait;
use crossterm::cursor::MoveTo;
use crossterm::style::{Print, PrintStyledContent};
use crossterm::QueueableCommand;
use crossterm::style::StyledContent;

use crate::tui::{style::StyleVariants, Rect, TuiResponse};
use std::net::IpAddr;

use futures::lock::Mutex;
use std::io::Stdout;
use std::sync::Arc;

type Out = Arc<Mutex<Stdout>>;

/// Trait for defining an item as listable in the UiList component
pub trait ListableItem {
    fn print(&self, is_selected: bool, max_length: usize) -> StyledContent<String>;
    fn size_text(&self, text: &str, max_length: usize) -> String {
        format!("{:max_length$.max_length$}", text.trim())
    }
}

impl ListableItem for IpAddr {
    fn print(&self, is_selected: bool, max_length: usize) -> StyledContent<String> {
        match is_selected {
            true => StyleVariants::get_styled_item(
                self.size_text(&self.to_string(), max_length),
                StyleVariants::Selected(true),
            ),
            false => StyleVariants::get_styled_item(
                self.size_text(&self.to_string(), max_length),
                StyleVariants::Selected(false),
            ),
        }
    }
}

#[async_trait]
/// Trait for defining an UIElement as a list
pub trait UiList<'a, T: ListableItem>: UiElement {
    fn new(&self, items: Arc<Mutex<Vec<T>>>, bounds: Rect, selected_item: usize) -> Self;
    async fn print(&self) -> Vec<StyledContent<String>>;
    fn bounds(&self) -> &Rect;
    fn get_selected_index(&self) -> usize;
}

pub struct AddressList<T>
where
    T: ListableItem + Send + Sync
{
    bounds: Rect,
    list: Arc<Mutex<Vec<T>>>,
    current: bool,
    selected_item: usize,
}

#[async_trait]
impl<'a, T: ListableItem + Send + Sync> UiList<'a, T> for AddressList<T> {
    fn new(&self, items: Arc<Mutex<Vec<T>>>, bounds: Rect, selected_item: usize) -> Self {
        AddressList {
            bounds,
            list: items,
            selected_item,
            current: false,
        }
    }
    fn get_selected_index(&self) -> usize {
        self.selected_item
    }

    async fn print(&self) -> Vec<StyledContent<String>> {
        self.list.lock().await
            .iter()
            .enumerate()
            .take(UiList::bounds(self).height())
            .map(
                |(index, address)| {
                    address.print(
                        index == self.get_selected_index(),
                        UiList::bounds(self).width(),
                    )
                } 
            )
            .collect()
    }
    fn bounds(&self) -> &Rect {
        &self.bounds
    }
}

#[async_trait]
pub trait UiElement {
    fn bounds(&self) -> &Rect;
    fn is_current(&self) -> bool;
    async fn render(self, out: Out) -> anyhow::Result<()>;
    fn get_header(&self) -> StyledContent<String>;
    fn get_next_line(&self, counter: u16) -> MoveTo{
        let bounds = self.bounds();
        let cols:u16 = bounds.cols.0 as u16;
        let rows:u16 = bounds.rows.0 as u16;
        MoveTo(cols, rows+counter)
    }
}

#[async_trait]
impl<T: ListableItem + Sync + Send > UiElement for AddressList<T> {
    fn is_current(&self) -> bool {
        self.current
    }

    fn bounds(&self) -> &Rect {
        &self.bounds
    }

    async fn render(self, out: Out) -> anyhow::Result<()> {
        let buffer: Vec<StyledContent<String>> = UiList::print(&self).await;
        let mut out = out.lock().await;
        out.queue(self.get_next_line(0))?;
        out.queue(PrintStyledContent(self.get_header()))?;
        for (line, content) in buffer.into_iter().enumerate() {
            out.queue(self.get_next_line(line as u16 + 1))?;
            out.queue(PrintStyledContent(content))?;
        }
        Ok(())
    }

    fn get_header(&self) -> StyledContent<String> {
        StyleVariants::get_styled_item(
            format!("{:^len$}", "Adress", len = UiElement::bounds(self).width()),
            StyleVariants::Header,
        )
    }
}

impl<T: ListableItem + Send + Sync + Clone> AddressList<T> {
    pub fn default(bounds: Rect, list: Arc<Mutex<Vec<T>>>, current: bool, selected_item: usize) -> Self {
        AddressList {
            list,
            current,
            bounds,
            selected_item,
        }
    }
    pub async fn get_selected_item(&self) -> Option<T> {
        let list = &self.list.lock().await;
        let item = list.get(self.get_selected_index());
        match item {
            Some(item) => return Some(item.clone()),
            None => None
        }

    }
}

impl ListableItem for TuiResponse {
    fn print(&self, is_selected: bool, max_length: usize) -> StyledContent<String> {
        match is_selected {
            true => {
                if let Some(response) = &self.http_response.content {
                    StyleVariants::get_styled_item(
                        self.size_text(response, max_length),
                        StyleVariants::Selected(true),
                    )
                } else {
                    StyleVariants::get_styled_item(
                        self.size_text("No response", max_length),
                        StyleVariants::Selected(true),
                    )
                }
            }
            false => {
                if let Some(response) = &self.http_response.content {
                    StyleVariants::get_styled_item(
                        self.size_text(response, max_length),
                        StyleVariants::Selected(false),
                    )
                } else {
                    StyleVariants::get_styled_item(
                        self.size_text("No response", max_length),
                        StyleVariants::Selected(false),
                    )
                }
            }
        }
    }
}

#[derive(Debug)]
pub struct ConnectionsList<T: ListableItem + Sync + Send> {
    bounds: Rect,
    list: Arc<Mutex<Vec<T>>>,
    current: bool,
    selected_item: usize,
}

#[async_trait]
impl<'a, T: ListableItem + Send + Sync> UiList<'a, T> for ConnectionsList<T> {
    fn new(&self, items: Arc<Mutex<Vec<T>>>, bounds: Rect, selected_item: usize) -> Self {
        ConnectionsList {
            bounds,
            list: items,
            current: false,
            selected_item,
        }
    }
    fn bounds(&self) -> &Rect {
        &self.bounds
    }
    fn get_selected_index(&self) -> usize {
        self.selected_item
    }
    async fn print(&self) -> Vec<StyledContent<String>> {
        let list = Arc::clone(&self.list);
        self.create_member_list(list, self.get_selected_index()).await
    }
}

#[async_trait]
impl<T: ListableItem + Send + Sync> UiElement for ConnectionsList<T> {
    fn bounds(&self) -> &Rect {
        &self.bounds
    }
    fn is_current(&self) -> bool {
        self.current
    }
    async fn render(self, out: Out) -> anyhow::Result<()> {
        let buffer = self.print().await;
        let mut out = out.lock().await;
        out.queue(self.get_next_line(0))?;
        out.queue(PrintStyledContent(self.get_header()))?;
        for (line, content) in buffer.into_iter().enumerate() {
            out.queue(self.get_next_line(line as u16 + 1))?;
            out.queue(PrintStyledContent(content))?;
        }

        Ok(())
    }
    fn get_header(&self) -> StyledContent<String> {
        StyleVariants::get_styled_item(
            format!(
                "{:^len$}",
                "Connections",
                len = UiElement::bounds(self).width()
            ),
            StyleVariants::Header,
        )
    }
}
impl<T: ListableItem + std::marker::Sync + std::marker::Send> ConnectionsList<T> {
    async fn create_member_list(
        &self,
        groups: Arc<Mutex<Vec<T>>>,
        _selectedgroup: usize,
    ) -> Vec<StyledContent<String>> {
        groups.lock().await
            .iter()
            .enumerate()
            .take(UiElement::bounds(self).height())
            .map(|(index, item)| {
                item.print(self.selected_item == index, UiList::bounds(self).width())
            })
            .collect()
    }

    pub fn default(list: Arc<Mutex<Vec<T>>>, bounds: Rect, current: bool, selected_item: usize) -> Self {
        ConnectionsList {
            list,
            bounds,
            current,
            selected_item,
        }
    }
}
