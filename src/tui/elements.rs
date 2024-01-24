use async_trait::async_trait;
use crossterm::QueueableCommand;
use crossterm::style::Print;
use crossterm::style::StyledContent;

use crate::tui::{style::StyleVariants, Rect, TuiResponse};
use std::net::IpAddr;

use futures::lock::Mutex;
use std::io::Stdout;
use std::sync::Arc;

type Out = Arc<Mutex<Stdout>>;

pub trait ListableItem {
    fn print(&self, is_selected: bool, max_length: usize) -> StyledContent<String>;
}

impl ListableItem for IpAddr {
    fn print(&self, is_selected: bool, max_length: usize) -> StyledContent<String> {
        match is_selected {
            true => StyleVariants::get_styled_item(
                format!("{:max_length$.max_length$}", self.to_string()),
                StyleVariants::Selected(true),
            ),
            false => StyleVariants::get_styled_item(
                format!("{:max_length$.max_length$}", self.to_string()),
                StyleVariants::Selected(false),
            ),
        }
    }
}

#[async_trait]
pub trait UiList<'a, T: ListableItem> {
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
            .map(
                |(index, address)| {
                    address.print(
                        index == self.get_selected_index(),
                        UiList::bounds(self).width(),
                    )
                }, //self.host.to_string()
            )
            .collect()
    }
    fn bounds(&self) -> &Rect {
        todo!()
    }
}

#[async_trait]
pub trait UiElement {
    fn bounds(&self) -> &Rect;
    fn is_current(&self) -> bool;
    async fn render(self, out: Out) -> anyhow::Result<()>;
    fn get_header(&self) -> StyledContent<String>;
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
        let buffer: Vec<StyledContent<String>> = UiList::print(&self).await
            .into_iter()
            .take(UiElement::bounds(&self).height())
            .map(|item| {
                StyleVariants::get_styled_item(
                    format!("{:max$.max$}", item, max = UiList::bounds(&self).width()),
                    StyleVariants::Selected(true),
                )
            })
            .collect();
        for line in buffer {
            out.lock().await.queue(Print(line))?;
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
    pub async fn get_selected_item(&self) -> T {
        let list = &self.list.lock().await;
         list[self.get_selected_index()].clone()
    }
}

impl ListableItem for TuiResponse {
    fn print(&self, is_selected: bool, max_length: usize) -> StyledContent<String> {
        match is_selected {
            true => {
                if let Some(response) = &self.http_response.content {
                    StyleVariants::get_styled_item(
                        format!("{:max_length$.max_length$}", response),
                        StyleVariants::Selected(true),
                    )
                } else {
                    StyleVariants::get_styled_item(
                        "No response".to_string(),
                        StyleVariants::Selected(true),
                    )
                }
            }
            false => {
                if let Some(response) = &self.http_response.content {
                    StyleVariants::get_styled_item(
                        format!("{:max_length$.max_length$}", response.to_string()),
                        StyleVariants::Selected(false),
                    )
                } else {
                    StyleVariants::get_styled_item(
                        "No response".to_string(),
                        StyleVariants::Selected(false),
                    )
                }
            }
        }
    }
}

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
        let lines: Vec<StyledContent<String>> = self
            .list.lock().await
            .iter()
            .enumerate()
            .take(UiElement::bounds(&self).height())
            .map(|(index, item)| {
                item.print(self.selected_item == index, UiList::bounds(&self).width())
            })
            .collect();
        out.lock().await.queue(Print(""))?;
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
        let groups = groups.lock().await;
        groups
            .iter()
            .take(UiElement::bounds(self).height())
            .map(|group| group.print(true, UiList::bounds(self).width()))
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

fn stash() {
    /*
    *
       let connection_list_bounds = Rect {
           rows: (window_size.rows.0, window_size.rows.1),
           cols: (window_size.cols.0, window_size.cols.1),
       };

       let addr_list_length: usize = connection_list_bounds.width().checked_div(3).unwrap_or(0);

       let (address_list_bounds, details_list_bounds): (Rect, Rect) = (
           Rect::new(
               (connection_list_bounds.cols.0 + 1, addr_list_length),
               connection_list_bounds.rows,
           ),
           Rect::new(
               (addr_list_length + 1, connection_list_bounds.cols.1),
               connection_list_bounds.rows,
           ),
       );
    *
    *
    *
    * */
}
