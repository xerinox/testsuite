use async_trait::async_trait;
use crossterm::cursor::MoveTo;
use crossterm::style::PrintStyledContent;
use crossterm::style::StyledContent;
use crossterm::QueueableCommand;

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

impl ListableItem for TuiResponse {
    fn print(&self, is_selected: bool, max_length: usize) -> StyledContent<String> {
        let content = self.get_response_as_line();
        match is_selected {
            true => StyleVariants::get_styled_item(
                self.size_text(&content, max_length),
                StyleVariants::Selected(true),
            ),
            false => StyleVariants::get_styled_item(
                self.size_text(&content, max_length),
                StyleVariants::Selected(false),
            ),
        }
    }
}

impl ListableItem for &str {
    fn print(&self, is_selected: bool, max_length: usize) -> StyledContent<String> {
        StyleVariants::get_styled_item(
            self.size_text(self, max_length),
            StyleVariants::Selected(is_selected),
        )
    }
    fn size_text(&self, text: &str, max_length: usize) -> String {
        format!("{:max_length$.max_length$}", text)
    }
}

#[async_trait]
/// Trait for defining an UIElement as a list
pub trait UiList<'a, T: ListableItem>: UiElement {
    fn new(items: Arc<Mutex<Vec<T>>>, bounds: Rect, current: bool, selected_item: usize) -> Self;
    async fn print(&self) -> Vec<StyledContent<String>>;
    fn bounds(&self) -> &Rect;
    fn get_selected_index(&self) -> usize;
}

#[derive(Debug)]
pub struct AddressList<T>
where
    T: ListableItem + Send + Sync,
{
    bounds: Rect,
    list: Arc<Mutex<Vec<T>>>,
    current: bool,
    selected_item: usize,
}

#[async_trait]
impl<'a, T: ListableItem + Send + Sync> UiList<'a, T> for AddressList<T> {
    fn new(items: Arc<Mutex<Vec<T>>>, bounds: Rect, current: bool, selected_item: usize) -> Self {
        AddressList {
            bounds,
            list: items,
            selected_item,
            current,
        }
    }
    fn get_selected_index(&self) -> usize {
        self.selected_item
    }

    async fn print(&self) -> Vec<StyledContent<String>> {
        let (height, width) = (UiList::bounds(self).height(), UiList::bounds(self).width());
        let mut buffer: Vec<StyledContent<String>> = vec![
            StyleVariants::get_styled_item(
                format!("{:width$.width$}", ""),
                StyleVariants::Selected(false)
            );
            height
        ];
        self.list
            .lock()
            .await
            .iter()
            .enumerate()
            .take(UiList::bounds(self).height())
            .for_each(|(index, address)| {
                buffer[index] = address.print(
                    index == self.get_selected_index(),
                    UiList::bounds(self).width(),
                );
            });
        buffer
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
    fn get_header(&self, current: bool) -> StyledContent<String>;
    fn get_next_line(&self, counter: usize) -> Option<MoveTo> {
        let next_line = self.bounds().rows.0 + counter;
        if next_line < self.bounds().rows.1 {
            Some(MoveTo(self.bounds().cols.0 as u16, next_line as u16))
        } else {
            None
        }
    }
}

#[async_trait]
impl<T: ListableItem + Sync + Send> UiElement for AddressList<T> {
    fn is_current(&self) -> bool {
        self.current
    }

    fn bounds(&self) -> &Rect {
        &self.bounds
    }

    async fn render(self, out: Out) -> anyhow::Result<()> {
        let buffer: Vec<StyledContent<String>> = UiList::print(&self).await;
        let mut out = out.lock().await;
        if let Some(next_line) = self.get_next_line(0) {
            out.queue(next_line)?;
            out.queue(PrintStyledContent(self.get_header(self.is_current())))?;
            for (line, content) in buffer.into_iter().enumerate() {
                if let Some(next_line) = self.get_next_line(line + 1) {
                    out.queue(next_line)?;
                    out.queue(PrintStyledContent(content))?;
                }
            }
        }
        Ok(())
    }

    fn get_header(&self, current: bool) -> StyledContent<String> {
        StyleVariants::get_styled_item(
            format!("{:^len$}", "Address", len = UiElement::bounds(self).width()),
            StyleVariants::Header(current),
        )
    }
}

impl<T: ListableItem + Send + Sync + Clone> AddressList<T> {
    pub fn default(
        bounds: Rect,
        list: Arc<Mutex<Vec<T>>>,
        current: bool,
        selected_item: usize,
    ) -> Self {
        AddressList {
            list,
            current,
            bounds,
            selected_item,
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
    fn new(items: Arc<Mutex<Vec<T>>>, bounds: Rect, current: bool, selected_item: usize) -> Self {
        ConnectionsList {
            bounds,
            list: items,
            current,
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
        self.create_member_list(list, self.get_selected_index())
            .await
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
        if let Some(next_line) = self.get_next_line(0) {
            out.queue(next_line)?;
            out.queue(PrintStyledContent(self.get_header(self.is_current())))?;
            for (line, content) in buffer.into_iter().enumerate() {
                if let Some(next_line) = self.get_next_line(line + 1) {
                    out.queue(next_line)?;
                    out.queue(PrintStyledContent(content))?;
                }
            }
        }
        Ok(())
    }
    fn get_header(&self, current: bool) -> StyledContent<String> {
        StyleVariants::get_styled_item(
            format!(
                "{:^len$}",
                "Connections",
                len = UiElement::bounds(self).width()
            ),
            StyleVariants::Header(current),
        )
    }
}
impl<T: ListableItem + std::marker::Sync + std::marker::Send> ConnectionsList<T> {
    async fn create_member_list(
        &self,
        groups: Arc<Mutex<Vec<T>>>,
        selectedgroup: usize,
    ) -> Vec<StyledContent<String>> {
        let (height, width) = (UiList::bounds(self).height(), UiList::bounds(self).width());
        let mut buffer = vec![
            StyleVariants::get_styled_item(
                format!("{:width$.width$}", ""),
                StyleVariants::Selected(false)
            );
            height
        ];
        groups
            .lock()
            .await
            .iter()
            .enumerate()
            .take(UiElement::bounds(self).height())
            .for_each(|(index, item)| {
                buffer[index] = item.print(selectedgroup == index, UiList::bounds(self).width())
            });
        buffer
    }

    pub fn default(
        list: Arc<Mutex<Vec<T>>>,
        bounds: Rect,
        current: bool,
        selected_item: usize,
    ) -> Self {
        ConnectionsList {
            list,
            bounds,
            current,
            selected_item,
        }
    }
}

pub struct ProgramHeader<'a> {
    pub bounds: &'a Rect,
}

#[async_trait]
impl UiElement for ProgramHeader<'_> {
    fn bounds(&self) -> &Rect {
        self.bounds
    }
    async fn render(self, out: Out) -> anyhow::Result<()> {
        let mut out = out.lock().await;

        out.queue(MoveTo(0, 0))?;
        out.queue(PrintStyledContent(self.get_header(false)))?;
        Ok(())
    }
    fn is_current(&self) -> bool {
        false
    }
    fn get_header(&self, _current: bool) -> StyledContent<String> {
        let width = self.bounds().width();
        StyleVariants::get_styled_item(format!("{:^width$}", "Testsuite"), StyleVariants::Title)
    }
    fn get_next_line(&self, _counter: usize) -> Option<MoveTo> {
        Some(MoveTo(0, 0))
    }
}

#[derive(Debug)]
pub struct DetailWindow<T> {
    bounds: Rect,
    current: bool,
    details: Arc<Mutex<Vec<T>>>,
    addr: String,
}

#[async_trait]
impl<T: ListableItem + std::marker::Sync + std::marker::Send> UiElement for DetailWindow<T> {
    fn bounds(&self) -> &Rect {
        &self.bounds
    }
    async fn render(self, out: Out) -> anyhow::Result<()> {
        let buffer: Vec<StyledContent<String>> = UiList::print(&self).await;
        let mut out = out.lock().await;
        if let Some(next_line) = self.get_next_line(0) {
            out.queue(next_line)?;
            out.queue(PrintStyledContent(self.get_header(self.is_current())))?;
            for (line, content) in buffer.into_iter().enumerate() {
                if let Some(next_line) = self.get_next_line(line + 1) {
                    out.queue(next_line)?;
                    out.queue(PrintStyledContent(content))?;
                }
            }
        }
        Ok(())
    }
    fn is_current(&self) -> bool {
        self.current
    }
    fn get_header(&self, current: bool) -> StyledContent<String> {
        let width = UiElement::bounds(self).width();
        StyleVariants::get_styled_item(
            format!("{:^width$}", &self.addr),
            StyleVariants::Header(current),
        )
    }
    fn get_next_line(&self, counter: usize) -> Option<MoveTo> {
        let next_line = self.bounds.rows.0 + counter;
        if next_line < self.bounds.rows.1 {
            Some(MoveTo(self.bounds.cols.0 as u16, next_line as u16))
        } else {
            None
        }
    }
}

#[async_trait]
impl<'a, T: ListableItem + Send + Sync> UiList<'a, T> for DetailWindow<T> {
    fn bounds(&self) -> &Rect {
        &self.bounds
    }

    fn new(items: Arc<Mutex<Vec<T>>>, bounds: Rect, current: bool, _selected_item: usize) -> Self {
        DetailWindow {
            bounds,
            current,
            details: items,
            addr: String::new(),
        }
    }
    async fn print(&self) -> Vec<StyledContent<String>> {
        let (height, width) = (UiList::bounds(self).height(), UiList::bounds(self).width());
        let mut buffer = vec![
            StyleVariants::get_styled_item(
                format!("{:width$.width$}", ""),
                StyleVariants::Selected(false)
            );
            height
        ];
        self.details
            .lock()
            .await
            .iter()
            .enumerate()
            .take(UiList::bounds(self).height())
            .for_each(|(index, line)| {
                buffer[index] = line.print(false, UiList::bounds(self).width());
            });
        buffer
    }
    fn get_selected_index(&self) -> usize {
        todo!()
    }
}

impl<T: ListableItem + Send + Sync + Clone> DetailWindow<T> {
    pub fn default(bounds: Rect, details: Arc<Mutex<Vec<T>>>, current: bool, addr: String) -> Self {
        DetailWindow {
            details,
            current,
            bounds,
            addr,
        }
    }
}
