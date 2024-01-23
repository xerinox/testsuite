use core::cmp::Ordering;
use itertools::Itertools;

use crate::tui::{Rect, TuiResponse, Screen};
use colored::{ColoredString, Colorize};
use crate::Connections;
use std::net::IpAddr;

use futures::lock::Mutex;
use std::sync::Arc;
use std::io::Stdout;
use crossterm::{
    cursor::MoveTo,
    QueueableCommand,
    style::Print};

type Out = Arc<Mutex<Stdout>>;

#[derive(Debug)]
pub struct ConnectionList<'a> {
    groups: Vec<ConnectionListGroup<'a>>,
    selected: (usize, usize),
    address_list_bounds: Rect,
    details_list_bounds: Rect,
    window_size: Rect,
    screen: Screen,
}

impl<'a> ConnectionList<'a> {
    pub fn default(
        groups: &Connections,
        window_size: Rect,
        selected: (usize, usize),
        screen: Screen,
    ) -> ConnectionList {
        let connection_list_bounds = Rect {
            rows: (window_size.rows.0, window_size.rows.1),
            cols: (window_size.cols.0, window_size.cols.1),
        };

        let addr_list_length: usize = connection_list_bounds.width().checked_div(3).unwrap_or(0);

        let (address_list_bounds, details_list_bounds): (Rect, Rect) = (
            Rect::new((1, addr_list_length), (1, connection_list_bounds.rows.1)),
            Rect::new(
                (addr_list_length + 1, connection_list_bounds.cols.1),
                (1, connection_list_bounds.rows.1),
            ),
        );

        let groups: Vec<ConnectionListGroup> = groups
            .iter()
            .map(|(host, lines)| ConnectionListGroup {
                host,
                lines: lines
                    .iter()
                    .map(|response| ConnectionListGroupLine { response })
                    .collect(),
            })
            .collect();

        let selected = (selected.0.max(0), selected.1.max(0));

        ConnectionList {
            address_list_bounds,
            details_list_bounds,
            window_size,
            selected,
            groups,
            screen,
        }
    }

    pub async fn render(&mut self, out: Out) -> anyhow::Result<()> {
        let mut connection_group_list: Vec<String> =
            self.groups.iter().map(|x| x.render()).collect();

        self.selected.0 = self
            .selected
            .0
            .max(0)
            .min(connection_group_list.len().checked_sub(1).unwrap_or(1));
        let mut connection_group_members =
            match self.groups.get(self.selected.0.checked_sub(1).unwrap_or(0)) {
                Some(connection_group_members) => {
                    connection_group_members.print_group_members(&self.details_list_bounds)
                }
                None => {
                    vec![]
                }
            };
        self.selected.1 = self
            .selected
            .1
            .max(0)
            .min(connection_group_members.len().checked_sub(1).unwrap_or(0));

        match connection_group_list
            .len()
            .cmp(&connection_group_members.len())
        {
            Ordering::Less => {
                connection_group_list.resize(connection_group_members.len(), String::new());
            }
            Ordering::Greater => {
                connection_group_members.resize(connection_group_members.len(), String::new());
            }
            Ordering::Equal => {}
        }

        let mut buffer: Vec<(Option<&str>, Option<&str>)> = vec![];

        for pair in connection_group_list
            .iter()
            .zip_longest(connection_group_members.iter())
        {
            match pair {
                itertools::EitherOrBoth::Both(list, members) => {
                    buffer.push((Some(list), Some(members)));
                }
                itertools::EitherOrBoth::Left(list) => {
                    buffer.push((Some(list), None));
                }
                itertools::EitherOrBoth::Right(members) => {
                    buffer.push((None, Some(members)));
                }
            }
        }
        let mut out = out.lock().await;
        for (line, line_content) in buffer
            .into_iter()
            .enumerate()
            .take(self.window_size.height())
        {
            let selected_lines = (
                line == self.selected.0,
                (line == self.selected.1) && self.screen == Screen::Details,
            );
            let line = line as u16;
            let line_text = self.print_line(line_content, selected_lines);
            out.queue(MoveTo(1, line))?;
            out.queue(Print(format!("{}", line_text.0)))?;
            out.queue(Print("|"))?;
            out.queue(Print(format!("{}", line_text.1)))?;
        }

        out.queue(crossterm::cursor::MoveToNextLine(1))?;
        out.queue(Print(format!("CurrentlySelectedItems:{:?}", self.selected)))?;
        out.queue(crossterm::cursor::MoveToNextLine(1))?;

        Ok(())
    }

    fn pad_and_truncate(str: &str, max_len: usize) -> String {
        let pad = "-".repeat(max_len);
        let mut buf = String::from(str);
        buf.push_str(&pad);
        buf.truncate(max_len);
        buf
    }

    fn print_line(
        &self,
        line: (Option<&str>, Option<&str>),
        selected: (bool, bool),
    ) -> (ColoredString, ColoredString) {
        let address_max_length = self.address_list_bounds.width();
        let members_max_length = self.details_list_bounds.width();

        let result = match line {
            (Some(list), Some(members)) => {
                let addr_max = address_max_length.checked_sub(list.len()).unwrap_or(0);
                let member_max = members_max_length.checked_sub(members.len()).unwrap_or(0);
                match selected {
                    (true, true) => (
                        Self::pad_and_truncate(list, addr_max).black().on_white(),
                        Self::pad_and_truncate(members, member_max)
                            .black()
                            .on_white(),
                    ),
                    (true, false) => (
                        Self::pad_and_truncate(list, addr_max).black().on_white(),
                        Self::pad_and_truncate(members, member_max).on_blue(),
                    ),
                    (false, true) => (
                        Self::pad_and_truncate(list, addr_max).on_blue(),
                        Self::pad_and_truncate(members, member_max)
                            .black()
                            .on_white(),
                    ),
                    (false, false) => (
                        Self::pad_and_truncate(list, addr_max).on_blue(),
                        Self::pad_and_truncate(members, member_max).on_blue(),
                    ),
                }
            }
            (Some(list), None) => {
                let addr_max = address_max_length.checked_sub(list.len()).unwrap_or(0);
                let member_max = members_max_length;
                match selected.0 {
                    true => (
                        Self::pad_and_truncate(list, addr_max).black().on_white(),
                        Self::pad_and_truncate("", member_max).on_blue(),
                    ),
                    false => (
                        Self::pad_and_truncate(list, addr_max).on_blue(),
                        Self::pad_and_truncate("", member_max).on_blue(),
                    ),
                }
            }
            (None, Some(members)) => {
                let addr_max = address_max_length;
                let member_max = members_max_length.checked_sub(members.len()).unwrap_or(0);
                match selected.1 {
                    true => (
                        Self::pad_and_truncate("", addr_max).normal(),
                        Self::pad_and_truncate(members, member_max)
                            .black()
                            .on_white(),
                    ),
                    false => (
                        Self::pad_and_truncate("", addr_max).normal(),
                        Self::pad_and_truncate(members, member_max).normal(),
                    ),
                }
            }
            _ => {
                let addr_pad = " ".repeat(address_max_length);
                let member_pad = " ".repeat(members_max_length);
                (addr_pad.on_blue(), member_pad.on_blue())
            }
        };
        (result.0, result.1)
    }

    pub fn selected(self) -> (usize, usize) {
        self.selected
    }

}

#[derive(Debug)]
struct ConnectionListGroupLine<'a> {
    response: &'a TuiResponse,
}

#[derive(Debug)]
struct ConnectionListGroup<'a> {
    host: &'a IpAddr,
    lines: Vec<ConnectionListGroupLine<'a>>,
}

impl<'a> ConnectionListGroup<'a> {
    fn render(&self) -> String {
        self.host.to_string()
    }

    fn print_group_members(&self, bounds: &Rect) -> Vec<String> {
        let max_length = bounds.height().min(self.lines.len());
        self.lines
            .iter()
            .take(max_length)
            .map(|line| {
                if let Some(content) = &line.response.http_response.content {
                    let content = content.trim();
                    let max_length = bounds.width().min(content.len());
                    let mut buf = String::new();
                    buf.push_str(&content[0..max_length]);
                    buf
                } else {
                    "No response".to_string()
                }
            })
            .collect()
    }
}
