use crossterm::style::*;

#[allow(dead_code)]
pub enum StyleVariants {
    Selected(bool),
    Header(bool),
    Title,
}

impl StyleVariants {
    pub fn get_styled_item(text: String, style_variant: StyleVariants) -> StyledContent<String> {
        let dark = Color::Rgb { r: 0, g: 0, b: 170 };
        let light = Color::Rgb {
            r: 0,
            g: 170,
            b: 170,
        };
        let header = Color::Rgb {
            r: 255,
            g: 255,
            b: 85,
        };
        let white = Color::Rgb {
            r: 255,
            g: 255,
            b: 255,
        };
        match style_variant {
            Self::Selected(selected) => match selected {
                true => style(text).with(dark).on(light),
                false => style(text).with(light).on(dark),
            },
            Self::Header(current) => match current {
                true => style(text).with(header).on(light),
                false => style(text).with(header).on(dark),
            },
            Self::Title => style(text).with(white).on(dark),
        }
    }
}
