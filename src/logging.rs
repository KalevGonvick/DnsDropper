use std::thread;
use env_logger::fmt::style::{Ansi256Color, Color, Style};
use log::Level;
use std::io::Write;
use crate::logging::HighlightStyle::{DebugHighlight, ErrorHighlight, InfoHighlight, TraceHighlight, WarnHighLight};

pub const DARK_GREY_HIGHLIGHT: Style = Style::new()
    .fg_color(Some(Color::Ansi256(Ansi256Color(8))));
pub const RED_HIGHLIGHT: Style = Style::new()
    .fg_color(Some(Color::Ansi256(Ansi256Color(9))));
pub const GREEN_HIGHLIGHT: Style = Style::new()
    .fg_color(Some(Color::Ansi256(Ansi256Color(10))));
pub const YELLOW_HIGHLIGHT: Style = Style::new()
    .fg_color(Some(Color::Ansi256(Ansi256Color(11))));
pub const BLUE_HIGHLIGHT: Style = Style::new()
    .fg_color(Some(Color::Ansi256(Ansi256Color(12))));
pub const PURPLE_HIGHLIGHT: Style = Style::new()
    .fg_color(Some(Color::Ansi256(Ansi256Color(13))));
pub const AQUA_HIGHLIGHT: Style = Style::new()
    .fg_color(Some(Color::Ansi256(Ansi256Color(14))));
pub const DEFAULT_STYLE: Style = BLUE_HIGHLIGHT;
pub const TRACE_STYLE: Style = PURPLE_HIGHLIGHT.bold();
pub const INFO_STYLE: Style = BLUE_HIGHLIGHT.bold();
pub const ERROR_STYLE: Style = RED_HIGHLIGHT.bold();
pub const DEBUG_STYLE: Style = GREEN_HIGHLIGHT.bold();
pub const WARN_STYLE: Style = YELLOW_HIGHLIGHT.bold();
pub const TIMESTAMP_STYLE: Style = DARK_GREY_HIGHLIGHT.underline();
pub const THREAD_NAME_STYLE: Style = AQUA_HIGHLIGHT.bold();

pub enum HighlightStyle {
    TraceHighlight,
    DebugHighlight,
    InfoHighlight,
    WarnHighLight,
    ErrorHighlight,
}

pub trait GetStyle: Sized {
    type Err;

    fn get_style(s: Self) -> Style;
}

impl GetStyle for HighlightStyle {
    type Err = ();

    fn get_style(s: Self) -> Style {
        return match s {
            TraceHighlight => TRACE_STYLE,
            DebugHighlight => DEBUG_STYLE,
            InfoHighlight => INFO_STYLE,
            WarnHighLight => WARN_STYLE,
            ErrorHighlight => ERROR_STYLE,
        };
    }
}

pub fn setup(level: &str) {
    let level_filter = env_logger::Env::default()
        .default_filter_or(level);

    env_logger::builder()
        .parse_env(level_filter)
        .format(|buf, record| {
            let level_colour: Style = match record.level() {
                Level::Error => {
                    HighlightStyle::get_style(ErrorHighlight)
                }
                Level::Warn => {
                    HighlightStyle::get_style(WarnHighLight)
                }
                Level::Info => {
                    HighlightStyle::get_style(InfoHighlight)
                }
                Level::Debug => {
                    HighlightStyle::get_style(DebugHighlight)
                }
                Level::Trace => {
                    HighlightStyle::get_style(TraceHighlight)
                }
            };

            let ts = buf.timestamp_millis();
            let lvl = record.level();
            let args = record.args();

            writeln!(
                buf,
                "[{TIMESTAMP_STYLE}{}{TIMESTAMP_STYLE:#}][{level_colour}{}{level_colour:#}][{THREAD_NAME_STYLE}{}{THREAD_NAME_STYLE:#}] {DEFAULT_STYLE}{}{DEFAULT_STYLE:#}",
                ts,
                lvl,
                thread::current().name().unwrap_or_default().to_ascii_uppercase(),
                args
            )
        }).init();
}

pub(crate) fn print_title() {
    let title_style = Style::new().fg_color(Some(Color::Ansi256(Ansi256Color(13))));
    let art = r#"
████████▄  ███▄▄▄▄      ▄████████      ████████▄     ▄████████  ▄██████▄     ▄███████▄    ▄███████▄    ▄████████    ▄████████
███   ▀███ ███▀▀▀██▄   ███    ███      ███   ▀███   ███    ███ ███    ███   ███    ███   ███    ███   ███    ███   ███    ███
███    ███ ███   ███   ███    █▀       ███    ███   ███    ███ ███    ███   ███    ███   ███    ███   ███    █▀    ███    ███
███    ███ ███   ███   ███             ███    ███  ▄███▄▄▄▄██▀ ███    ███   ███    ███   ███    ███  ▄███▄▄▄      ▄███▄▄▄▄██▀
███    ███ ███   ███ ▀███████████      ███    ███ ▀▀███▀▀▀▀▀   ███    ███ ▀█████████▀  ▀█████████▀  ▀▀███▀▀▀     ▀▀███▀▀▀▀▀
███    ███ ███   ███          ███      ███    ███ ▀███████████ ███    ███   ███          ███          ███    █▄  ▀███████████
███   ▄███ ███   ███    ▄█    ███      ███   ▄███   ███    ███ ███    ███   ███          ███          ███    ███   ███    ███
████████▀   ▀█   █▀   ▄████████▀       ████████▀    ███    ███  ▀██████▀   ▄████▀       ▄████▀        ██████████   ███    ███
                                                    ███    ███                                                     ███    ███
"#;
    println!("{title_style}{}{title_style:#}", art);
}