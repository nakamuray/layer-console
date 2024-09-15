mod config;
mod layer_console;
mod util;

use gtk::gdk;
use gtk::gio::ApplicationCommandLine;
use gtk::gio::ApplicationFlags;
use gtk::glib::OptionArg;
use gtk::glib::OptionFlags;
use gtk::prelude::*;
use gtk::Application;
use gtk4_layer_shell::LayerShell;

pub const G_LOG_DOMAIN: &str = "layer-console";

fn on_commandline(app: &Application, command_line: &ApplicationCommandLine) -> i32 {
    let options = command_line.options_dict();
    let position = if options.contains("top") {
        Some(config::Position::Top)
    } else if options.contains("bottom") {
        Some(config::Position::Bottom)
    } else if options.contains("left") {
        Some(config::Position::Left)
    } else if options.contains("right") {
        Some(config::Position::Right)
    } else {
        None
    };
    let keyboard_mode = if options.contains("exclusive") {
        Some(config::KeyboardMode::Exclusive)
    } else if options.contains("on-demand") {
        Some(config::KeyboardMode::OnDemand)
    } else {
        None
    };

    let rows = options.lookup::<i32>("rows").unwrap().map(|i| i.into());
    let columns = options.lookup::<i32>("columns").unwrap().map(|i| i.into());

    if let Some(win) = app.active_window() {
        if let Ok(win) = win.clone().downcast::<layer_console::LayerConsoleWindow>() {
            if let Some(font) = options.lookup::<String>("font").unwrap() {
                win.set_font(&font);
            }
            if let Some(position) = position {
                win.set_position(position.as_position());
            }
            if let Some(keyboard_mode) = keyboard_mode {
                win.set_keyboard_mode(keyboard_mode.as_keyboard_mode());
            }
            match (columns, rows) {
                (None, None) => (),
                _ => win.set_terminal_size(columns, rows),
            }
            win.toggle();
        } else {
            panic!("failed to downcast {:?}", win);
        }
        return 0;
    }
    let win = layer_console::LayerConsoleWindow::new(app);

    let config = config::load_config(options.lookup::<std::path::PathBuf>("config").unwrap());

    let working_directory = options
        .lookup::<String>("working-directory")
        .unwrap()
        .or(config.working_directory);
    let rows = rows.or(config.rows);
    let columns = columns.or(config.columns);
    let font = options.lookup::<String>("font").unwrap().or(config.font);
    let position = position.or(config.position);
    let keyboard_mode = keyboard_mode.or(config.keyboard_mode);
    let shell = config.shell.unwrap_or_else(|| util::get_user_shell());

    win.set_working_directory(working_directory);
    win.set_terminal_size(columns, rows);
    if let Some(font) = font {
        win.set_font(&font);
    }
    if let Some(position) = position {
        win.set_position(position.as_position());
    }
    if let Some(keyboard_mode) = keyboard_mode {
        win.set_keyboard_mode(keyboard_mode.as_keyboard_mode());
    }

    if options.contains("command") {
        let mut args = command_line
            .arguments()
            .iter()
            .skip(1)
            .map(|s| s.to_string_lossy().to_string())
            .collect::<Vec<_>>();
        if let Some(index) = args.iter().position(|s| s == "--") {
            // remove first occurence of "--"
            args.remove(index);
        }
        win.spawn(&args.iter().map(String::as_str).collect::<Vec<_>>());
    } else {
        win.spawn(&[&shell]);
    }
    win.present();
    return 0;
}

fn add_main_options(app: &Application) {
    app.add_main_option(
        "command",
        b'e'.into(),
        OptionFlags::NONE,
        OptionArg::None,
        "Execute arguments inside the terminal",
        None,
    );
    app.add_main_option(
        "working-directory",
        b'\0'.into(),
        OptionFlags::NONE,
        OptionArg::String,
        "Set the wrking directory",
        Some("DIRNAME"),
    );
    app.add_main_option(
        "rows",
        b'r'.into(),
        OptionFlags::NONE,
        OptionArg::Int,
        "Set rows",
        Some("ROWS"),
    );
    app.add_main_option(
        "columns",
        b'c'.into(),
        OptionFlags::NONE,
        OptionArg::Int,
        "Set columns",
        Some("COLUMNS"),
    );
    app.add_main_option(
        "font",
        b'f'.into(),
        OptionFlags::NONE,
        OptionArg::String,
        "Set font",
        Some("FONT"),
    );
    app.add_main_option(
        "top",
        b'\0'.into(),
        OptionFlags::NONE,
        OptionArg::None,
        "Set position top (default)",
        None,
    );
    app.add_main_option(
        "bottom",
        b'\0'.into(),
        OptionFlags::NONE,
        OptionArg::None,
        "Set position bottom",
        None,
    );
    app.add_main_option(
        "left",
        b'\0'.into(),
        OptionFlags::NONE,
        OptionArg::None,
        "Set position left",
        None,
    );
    app.add_main_option(
        "right",
        b'\0'.into(),
        OptionFlags::NONE,
        OptionArg::None,
        "Set position right",
        None,
    );
    app.add_main_option(
        "exclusive",
        b'\0'.into(),
        OptionFlags::NONE,
        OptionArg::None,
        "Set keyboard mode exclusive",
        None,
    );
    app.add_main_option(
        "on-demand",
        b'\0'.into(),
        OptionFlags::NONE,
        OptionArg::None,
        "Set keyboard mode on-demand (default)",
        None,
    );
    app.add_main_option(
        "config",
        b'\0'.into(),
        OptionFlags::NONE,
        OptionArg::Filename,
        "config file path",
        Some("CONFIG"),
    );
}

fn main() {
    let mut application_id = "org.u7fa9.layer-console";
    if cfg!(debug_assertions) {
        // change application_id if it is not a release build
        application_id = "org.u7fa9.layer-console.debug";
    }
    let app = Application::builder()
        .application_id(application_id)
        .flags(ApplicationFlags::CAN_OVERRIDE_APP_ID | ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();
    add_main_options(&app);
    app.connect_startup(|app| {
        let display = gdk::Display::default().expect("can't get display");
        let provider = gtk::CssProvider::new();
        provider.load_from_string(
            r#"
            window, vte-terminal {
                background-color: transparent;
            }
            vte-terminal {
                border-style: solid;
                border-color: grey;
            }
            vte-terminal.top {
                border-width: 0px 1px 1px 1px;
                padding-top: 0.5em;
            }
            vte-terminal.bottom {
                border-width: 1px 1px 0 1px;
                padding-bottom: 0.5em;
            }
            vte-terminal.left {
                border-width: 1px 1px 1px 0;
                padding-left: 0.5em;
            }
            vte-terminal.right {
                border-width: 1px 0px 1px 1px;
                padding-right: 0.5em;
            }
        "#,
        );
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );

        app.set_accels_for_action("win.copy", &["<Shift><Primary>c"]);
        app.set_accels_for_action("win.paste", &["<Shift><Primary>v"]);
        app.set_accels_for_action("win.fullscreen", &["F11"]);
    });
    app.connect_command_line(on_commandline);

    app.run_with_args(&std::env::args().collect::<Vec<_>>());
}
