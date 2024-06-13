mod layer_console;
mod util;

use gtk::gdk;
use gtk::gio::ApplicationCommandLine;
use gtk::gio::ApplicationFlags;
use gtk::glib::OptionArg;
use gtk::glib::OptionFlags;
use gtk::prelude::*;
use gtk::Application;

fn on_activate(app: &Application) {
    if let Some(win) = app.active_window() {
        if let Ok(win) = win.clone().downcast::<layer_console::LayerConsoleWindow>() {
            win.toggle();
        } else {
            panic!("failed to downcast {:?}", win);
        }
    } else {
        let win = layer_console::LayerConsoleWindow::new(app);
        win.spawn(&[&util::get_user_shell()]);
        win.present();
    }
}

fn on_commandline(app: &Application, command_line: &ApplicationCommandLine) -> i32 {
    let options = command_line.options_dict();

    if let Some(win) = app.active_window() {
        if let Ok(win) = win.clone().downcast::<layer_console::LayerConsoleWindow>() {
            win.toggle();
        } else {
            panic!("failed to downcast {:?}", win);
        }
        return 0;
    }
    let win = layer_console::LayerConsoleWindow::new(app);
    win.set_working_directory(options.lookup::<String>("working-directory").unwrap());
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
        win.spawn(&[&util::get_user_shell()]);
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
                border-width: 1px 1px 0;
                border-color: grey;
                padding-bottom: 0.5em;
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
    });
    app.connect_activate(on_activate);
    app.connect_command_line(on_commandline);

    app.run_with_args(&std::env::args().collect::<Vec<_>>());
}
