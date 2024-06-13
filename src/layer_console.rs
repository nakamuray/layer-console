use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gio, glib};

mod imp {
    use gdk::RGBA;
    use glib::GString;
    use gtk::subclass::prelude::*;
    use gtk::{gdk, gio, glib, pango};
    use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
    use std::cell::RefCell;
    use vte4::prelude::*;

    #[derive(glib::Properties, Default)]
    #[properties(wrapper_type = super::LayerConsoleWindow)]
    pub struct LayerConsoleWindow {
        stack: gtk::Stack,
        terminal: vte4::Terminal,
        #[property(get, set, nullable)]
        working_directory: RefCell<Option<String>>,
    }

    impl LayerConsoleWindow {
        pub fn spawn(&self, args: &[&str]) {
            self.terminal.spawn_async(
                vte4::PtyFlags::DEFAULT,
                self.working_directory.borrow().as_ref().map(|s| s.as_str()),
                args,
                &[],
                glib::SpawnFlags::DEFAULT,
                || {},
                -1,
                gio::Cancellable::NONE,
                |_| {},
            );
        }
        fn connect_signals(&self) {
            let window = self.obj();
            self.terminal.connect_child_exited(
                glib::clone!(@weak window => move |_vte, _status| window.close()),
            );
            self.stack.connect_transition_running_notify(|stack| {
                if !stack.is_transition_running()
                    && stack.visible_child_name() == Some(GString::from("empty"))
                {
                    stack.parent().unwrap().set_visible(false);
                }
            });
            let stack = self.stack.clone();
            self.obj()
                .connect_show(glib::clone!(@weak stack => move |_| {
                    stack.set_transition_type(gtk::StackTransitionType::SlideUp);
                    stack.set_visible_child_name("terminal");
                }));
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
                self.stack
                    .set_transition_type(gtk::StackTransitionType::SlideDown);
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

            let window = self.obj();
            window.init_layer_shell();
            window.set_layer(Layer::Top);
            window.set_anchor(Edge::Bottom, true);
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

            self.terminal.set_size(100, 25);
            self.terminal
                .set_font(Some(&pango::FontDescription::from_string(
                    "Noto Sans Mono CJK JP 13",
                )));
            self.set_terminal_colors();
            self.terminal.set_bold_is_bright(true);
            self.terminal.grab_focus();
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
}