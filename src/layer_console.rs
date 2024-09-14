use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gio, glib};
use gtk4_layer_shell::Edge;

pub const DEFAULT_FONT: &str = "Monospace 13";
pub const DEFAULT_ROWS: i64 = 25;
pub const DEFAULT_COLUMNS: i64 = 100;

#[derive(Default, Debug, Eq, PartialEq, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "LayerConsolePosition")]
pub enum Position {
    #[default]
    Top = 0,
    Bottom = 1,
    Left = 2,
    Right = 3,
}

impl Position {
    pub fn to_edge(&self) -> Edge {
        match self {
            Position::Top => Edge::Top,
            Position::Bottom => Edge::Bottom,
            Position::Left => Edge::Left,
            Position::Right => Edge::Right,
        }
    }
}

mod imp {
    use super::{Position, DEFAULT_COLUMNS, DEFAULT_FONT, DEFAULT_ROWS};
    use crate::G_LOG_DOMAIN;
    use const_format::concatcp;
    use gdk::RGBA;
    use glib::GString;
    use gtk::gdk::AppLaunchContext;
    use gtk::gio::{AppInfo, SimpleAction};
    use gtk::subclass::prelude::*;
    use gtk::{gdk, gio, glib, pango};
    use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
    use std::cell::{Cell, RefCell};
    use std::collections::HashSet;
    use vte4::prelude::*;

    const PCRE2_MULTILINE: u32 = 0x00000400;

    // regex for url from kgx
    const USERCHARS: &str = "-[:alnum:]";
    const USERCHARS_CLASS: &str = concatcp!("[", USERCHARS, "]");
    const PASSCHARS_CLASS: &str = "[-[:alnum:]\\Q,?;.:/!%$^*&~\"#'\\E]";
    const HOSTCHARS_CLASS: &str = "[-[:alnum:]]";
    const HOST: &str = concatcp!(HOSTCHARS_CLASS, "+(\\.", HOSTCHARS_CLASS, "+)*");
    const PORT: &str = "(?:\\:[[:digit:]]{1,5})?";
    const PATHCHARS_CLASS: &str = "[-[:alnum:]\\Q_$.+!*,;:@&=?/~#%\\E]";
    const PATHTERM_CLASS: &str = "[^\\Q]'.}>) \t\r\n,\"\\E]";
    const SCHEME: &str = concat!(
        "(?:news:|telnet:|nntp:|file:\\/|https?:|ftps?:|sftp:|webcal:\n",
        "|irc:|sftp:|ldaps?:|nfs:|smb:|rsync:|",
        "ssh:|rlogin:|telnet:|git:\n",
        "|git\\+ssh:|bzr:|bzr\\+ssh:|svn:|svn\\",
        "+ssh:|hg:|mailto:|magnet:)"
    );
    const USERPASS: &str = concatcp!(USERCHARS_CLASS, "+(?:", PASSCHARS_CLASS, "+)?");
    const URLPATH: &str = concatcp!(
        "(?:(/",
        PATHCHARS_CLASS,
        "+(?:[(]",
        PATHCHARS_CLASS,
        "*[)])*",
        PATHCHARS_CLASS,
        "*)*",
        PATHTERM_CLASS,
        ")?"
    );

    const LINKS: [&str; 5] = [
        concatcp!(SCHEME, "//(?:", USERPASS, "\\@)?", HOST, PORT, URLPATH),
        concatcp!("(?:www|ftp)", HOSTCHARS_CLASS, "*\\.", HOST, PORT, URLPATH),
        concatcp!(
            "(?:callto:|h323:|sip:)",
            USERCHARS_CLASS,
            "[",
            USERCHARS,
            ".]*(?:",
            PORT,
            "/[a-z0-9]+)?\\@",
            HOST
        ),
        concatcp!(
            "(?:mailto:)?",
            USERCHARS_CLASS,
            "[",
            USERCHARS,
            ".]*\\@",
            HOSTCHARS_CLASS,
            "+\\.",
            HOST
        ),
        "(?:news:|man:|info:)[-[:alnum:]\\Q^_{|}~!\"#$%&'()*+,./;:=?`\\E]+",
    ];

    #[derive(glib::Properties, Default, Debug)]
    #[properties(wrapper_type = super::LayerConsoleWindow)]
    pub struct LayerConsoleWindow {
        stack: gtk::Stack,
        terminal: vte4::Terminal,
        #[property(get, set, nullable)]
        working_directory: RefCell<Option<String>>,
        #[property(get, set = Self::set_position, builder(Position::Top))]
        position: Cell<Position>,

        columns: Cell<i64>,
        rows: Cell<i64>,
        is_fullscreen: Cell<bool>,
        match_ids: std::cell::RefCell<HashSet<i32>>,
    }

    impl LayerConsoleWindow {
        fn set_position(&self, position: Position) {
            if self.position.get() == position {
                return;
            }
            self.position.replace(position);
            self.set_anchors();
            self.set_css_class();
        }
        pub fn set_font(&self, font: &str) {
            self.terminal
                .set_font(Some(&pango::FontDescription::from_string(font)));
        }
        fn set_css_class(&self) {
            let class_name = match self.position.get() {
                Position::Top => "top",
                Position::Bottom => "bottom",
                Position::Left => "left",
                Position::Right => "right",
            };
            self.terminal.set_css_classes(&[class_name]);
        }
        pub fn set_terminal_size(&self, columns: Option<i64>, rows: Option<i64>) {
            let columns = columns.unwrap_or_else(|| self.terminal.column_count());
            let rows = rows.unwrap_or_else(|| self.terminal.row_count());
            self.columns.replace(columns);
            self.rows.replace(rows);
            self.terminal.set_size(columns, rows);
        }
        pub fn fullscreen(&self) {
            if self.is_fullscreen.get() {
                self.set_anchors();
                self.terminal.set_size(self.columns.get(), self.rows.get());
                self.is_fullscreen.replace(false);
            } else {
                let window = self.obj();
                for edge in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
                    window.set_anchor(edge, true);
                }
                self.is_fullscreen.replace(true);
            }
        }
        pub fn spawn(&self, args: &[&str]) {
            self.terminal.spawn_async(
                vte4::PtyFlags::DEFAULT,
                self.working_directory.borrow().as_deref(),
                args,
                &[],
                glib::SpawnFlags::DEFAULT,
                || {},
                -1,
                gio::Cancellable::NONE,
                |_| {},
            );
        }
        fn get_url(&self, x: f64, y: f64) -> Option<GString> {
            if let Some(hyperlink) = self.terminal.check_hyperlink_at(x, y) {
                return Some(hyperlink);
            } else if let (Some(url), id) = self.terminal.check_match_at(x, y) {
                if self.match_ids.borrow().contains(&id) {
                    return Some(url);
                }
            }
            None
        }
        fn open_url(&self, url: &str) {
            if let Err(e) = AppInfo::launch_default_for_uri(url, AppLaunchContext::NONE) {
                glib::g_warning!(
                    G_LOG_DOMAIN,
                    "failed to launch app for uri ({}): {}",
                    url,
                    e
                );
            }
        }
        fn connect_signals(&self) {
            let window = self.obj();
            self.terminal.connect_child_exited(glib::clone!(
                #[weak]
                window,
                move |_vte, _status| window.close()
            ));
            self.stack.connect_transition_running_notify(|stack| {
                if !stack.is_transition_running()
                    && stack.visible_child_name() == Some(GString::from("empty"))
                {
                    stack.parent().unwrap().set_visible(false);
                }
            });
            let stack = self.stack.clone();
            self.obj().connect_show(glib::clone!(
                #[weak(rename_to = this)]
                self,
                #[weak]
                stack,
                move |_| {
                    let transition_type = match this.position.get() {
                        Position::Top => gtk::StackTransitionType::SlideDown,
                        Position::Bottom => gtk::StackTransitionType::SlideUp,
                        Position::Left => gtk::StackTransitionType::SlideRight,
                        Position::Right => gtk::StackTransitionType::SlideLeft,
                    };
                    stack.set_transition_type(transition_type);
                    stack.set_visible_child_name("terminal");
                }
            ));
        }
        fn setup_actions(&self) {
            let window = self.obj();

            let action = SimpleAction::new("copy", None);
            action.connect_activate(glib::clone!(
                #[weak(rename_to = this)]
                self,
                move |_action, _parameter| {
                    this.terminal.copy_clipboard_format(vte4::Format::Text);
                }
            ));
            window.add_action(&action);

            let action = SimpleAction::new("paste", None);
            action.connect_activate(glib::clone!(
                #[weak(rename_to = this)]
                self,
                move |_action, _parameter| {
                    this.terminal.paste_clipboard();
                }
            ));
            window.add_action(&action);

            let action = SimpleAction::new("fullscreen", None);
            action.connect_activate(glib::clone!(
                #[weak(rename_to = this)]
                self,
                move |_action, _parameter| {
                    this.fullscreen();
                }
            ));
            window.add_action(&action);
        }
        pub fn set_anchors(&self) {
            let window = self.obj();
            for edge in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
                if edge == self.position.get().to_edge() {
                    window.set_anchor(edge, true);
                } else {
                    window.set_anchor(edge, false);
                }
            }
        }
        fn set_terminal_colors(&self) {
            // color scheme from alacritty
            let foreground = RGBA::parse("#d8d8d8").unwrap();
            let mut background = RGBA::parse("#181818").unwrap();
            background.set_alpha(0.8);
            let pallet = [
                &RGBA::parse("#181818").unwrap(), // Black
                &RGBA::parse("#ac4242").unwrap(), // Red
                &RGBA::parse("#90a959").unwrap(), // Green
                &RGBA::parse("#f4bf75").unwrap(), // Yellow
                &RGBA::parse("#6a9fb5").unwrap(), // Blue
                &RGBA::parse("#aa759f").unwrap(), // Magenta
                &RGBA::parse("#75b5aa").unwrap(), // Cyan
                &RGBA::parse("#d8d8d8").unwrap(), // White
                &RGBA::parse("#6b6b6b").unwrap(), // Bright Black
                &RGBA::parse("#c55555").unwrap(), // Bright Red
                &RGBA::parse("#aac474").unwrap(), // Bright Green
                &RGBA::parse("#feca88").unwrap(), // Bright Yellow
                &RGBA::parse("#82b8c8").unwrap(), // Bright Blue
                &RGBA::parse("#c28cb8").unwrap(), // Bright Magenta
                &RGBA::parse("#93d3c3").unwrap(), // Bright Cyan
                &RGBA::parse("#f8f8f8").unwrap(), // Bright White
            ];
            self.terminal
                .set_colors(Some(&foreground), Some(&background), &pallet);
        }
        pub fn toggle(&self) {
            let window = self.obj();
            if window.is_visible() {
                let transition_type = match self.position.get() {
                    Position::Top => gtk::StackTransitionType::SlideUp,
                    Position::Bottom => gtk::StackTransitionType::SlideDown,
                    Position::Left => gtk::StackTransitionType::SlideLeft,
                    Position::Right => gtk::StackTransitionType::SlideRight,
                };
                self.stack.set_transition_type(transition_type);
                self.stack.set_visible_child_name("empty");
            } else {
                window.present();
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LayerConsoleWindow {
        const NAME: &'static str = "LayerConsoleWindow";
        type Type = super::LayerConsoleWindow;
        type ParentType = gtk::ApplicationWindow;
    }

    #[glib::derived_properties]
    impl ObjectImpl for LayerConsoleWindow {
        fn constructed(&self) {
            self.parent_constructed();

            self.connect_signals();
            self.setup_actions();

            let window = self.obj();
            window.init_layer_shell();
            window.set_layer(Layer::Top);
            self.set_anchors();
            self.set_css_class();
            // XXX: doesn't niri support OnDemand?
            //window.set_keyboard_mode(KeyboardMode::OnDemand);
            window.set_keyboard_mode(KeyboardMode::Exclusive);

            let empty = gtk::Box::new(gtk::Orientation::Vertical, 0);
            self.stack.add_named(&empty, Some("empty"));
            self.stack.set_transition_duration(300);
            window.set_child(Some(&self.stack));

            let scrolled = gtk::ScrolledWindow::builder()
                .vexpand(true)
                .propagate_natural_height(true)
                .propagate_natural_width(true)
                .hscrollbar_policy(gtk::PolicyType::Never)
                .build();
            scrolled.set_child(Some(&self.terminal));
            self.stack.add_named(&scrolled, Some("terminal"));

            self.terminal.set_size(DEFAULT_COLUMNS, DEFAULT_ROWS);
            self.set_font(DEFAULT_FONT);
            self.set_terminal_colors();
            self.terminal.set_bold_is_bright(true);
            self.terminal.grab_focus();
            self.terminal.set_allow_hyperlink(true);

            for l in LINKS {
                match vte4::Regex::for_match(l, PCRE2_MULTILINE) {
                    Err(e) => {
                        glib::g_warning!(G_LOG_DOMAIN, "link regex failed: {}", e);
                    }
                    Ok(regex) => {
                        let id = self.terminal.match_add_regex(&regex, 0);
                        self.terminal.match_set_cursor_name(id, "pointer");
                        self.match_ids.borrow_mut().insert(id);
                    }
                }
            }
            let gesture = gtk::GestureClick::new();
            gesture.set_button(1);
            gesture.connect_pressed(glib::clone!(
                #[weak(rename_to = this)]
                self,
                move |gesture, n, x, y| {
                    if n > 1 {
                        gesture.set_state(gtk::EventSequenceState::Denied);
                        return;
                    }
                    if !gesture
                        .current_event_state()
                        .contains(gdk::ModifierType::CONTROL_MASK)
                    {
                        gesture.set_state(gtk::EventSequenceState::Denied);
                        return;
                    }
                    if let Some(url) = this.get_url(x, y) {
                        this.open_url(&url);
                        gesture.set_state(gtk::EventSequenceState::Claimed);
                        return;
                    }
                    gesture.set_state(gtk::EventSequenceState::Denied);
                }
            ));
            self.terminal.add_controller(gesture);

            self.terminal
                .connect_hyperlink_hover_uri_notify(|terminal| {
                    terminal.set_tooltip_text(terminal.hyperlink_hover_uri().as_deref());
                });
        }
    }
    impl WidgetImpl for LayerConsoleWindow {}
    impl WindowImpl for LayerConsoleWindow {}
    impl ApplicationWindowImpl for LayerConsoleWindow {}
}

glib::wrapper! {
    pub struct LayerConsoleWindow(ObjectSubclass<imp::LayerConsoleWindow>)
        @extends gtk::Widget, gtk::Window, gtk::ApplicationWindow,
        @implements gio::ActionMap, gio::ActionGroup;
}

impl LayerConsoleWindow {
    pub fn new<P: IsA<gtk::Application>>(app: &P) -> Self {
        glib::Object::builder().property("application", app).build()
    }
    pub fn toggle(&self) {
        self.imp().toggle();
    }
    pub fn spawn(&self, args: &[&str]) {
        self.imp().spawn(args)
    }
    pub fn set_font(&self, font: &str) {
        self.imp().set_font(font);
    }
    pub fn set_terminal_size(&self, columns: Option<i64>, rows: Option<i64>) {
        self.imp().set_terminal_size(columns, rows);
    }
    pub fn fullscreen(&self) {
        self.imp().fullscreen();
    }
}
