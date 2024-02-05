use std::thread;
use env_logger::fmt::style::{Ansi256Color, Color, Style};
use log::Level;
use std::io::Write;
use crate::logging::HighlightStyle::{DebugHighlight, ErrorHighlight, InfoHighlight, TraceHighlight, WarnHighLight};

const TIMESTAMP_STYLE: Style = Style::new()
    .fg_color(Some(Color::Ansi256(Ansi256Color(8))))
    .underline();

const THREAD_NAME_STYLE: Style = Style::new()
    .fg_color(Some(Color::Ansi256(Ansi256Color(14))));

const MODULE_INFO_STYLE: Style = Style::new()
    .fg_color(Some(Color::Ansi256(Ansi256Color(11))))
    .italic();

pub enum HighlightStyle {
    TraceHighlight,
    DebugHighlight,
    InfoHighlight,
    WarnHighLight,
    ErrorHighlight,
}

pub fn setup(level: &str) {
    let level_filter = env_logger::Env::default()
        .default_filter_or(level);

    env_logger::builder()
        .parse_env(level_filter)
        .format(|buf, record| {
            let level_colour: Style = match record.level() {
                Level::Error => {
                    get_highlight_style(ErrorHighlight)
                }
                Level::Warn => {
                    get_highlight_style(WarnHighLight)
                }
                Level::Info => {
                    get_highlight_style(InfoHighlight)
                }
                Level::Debug => {
                    get_highlight_style(DebugHighlight)
                }
                Level::Trace => {
                    get_highlight_style(TraceHighlight)
                }
            };

            let ts = buf.timestamp_millis();
            let mod_path = record.module_path();
            let mod_line = record.line();
            let lvl = record.level();
            let args = record.args();

            writeln!(buf, "[{TIMESTAMP_STYLE}{}{TIMESTAMP_STYLE:#}][{THREAD_NAME_STYLE}{}{THREAD_NAME_STYLE:#}][{level_colour}{}{level_colour:#}][{MODULE_INFO_STYLE}{}.rs::{}{MODULE_INFO_STYLE:#}] {}",
                     ts,
                     thread::current().name().unwrap().to_ascii_uppercase(),
                     lvl,
                     mod_path.unwrap(),
                     mod_line.unwrap(),
                     args)
        }).init();
}

pub fn get_highlight_style(style: HighlightStyle) -> Style {
    return match style {
        TraceHighlight => {
            Style::new().fg_color(Some(Color::Ansi256(Ansi256Color(13)))).bold()
        }
        DebugHighlight => {
            Style::new().fg_color(Some(Color::Ansi256(Ansi256Color(10)))).bold()
        }
        InfoHighlight => {
            Style::new().fg_color(Some(Color::Ansi256(Ansi256Color(12)))).bold()
        }
        WarnHighLight => {
            Style::new().fg_color(Some(Color::Ansi256(Ansi256Color(11)))).bold()
        }
        ErrorHighlight => {
            Style::new().fg_color(Some(Color::Ansi256(Ansi256Color(9)))).bold()
        }
    };
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