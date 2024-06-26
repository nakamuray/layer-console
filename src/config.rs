use gtk::prelude::*;
use gtk::{gio, glib};
use serde::Deserialize;

use crate::layer_console;
use crate::G_LOG_DOMAIN;

const CONFIG_DIR_NAME: &str = "layer-console";
const CONFIG_FILE_NAME: &str = "config.toml";

#[derive(Debug, Default, Deserialize)]
pub struct Config {
    pub shell: Option<String>,
    pub working_directory: Option<String>,
    pub rows: Option<i64>,
    pub columns: Option<i64>,
    pub font: Option<String>,
    pub position: Option<Position>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Position {
    Top,
    Bottom,
    Left,
    Right,
}

impl Position {
    pub fn as_position(&self) -> layer_console::Position {
        match self {
            Position::Top => layer_console::Position::Top,
            Position::Bottom => layer_console::Position::Bottom,
            Position::Left => layer_console::Position::Left,
            Position::Right => layer_console::Position::Right,
        }
    }
}

fn default_config_path() -> std::path::PathBuf {
    let mut config_path = glib::user_config_dir();
    config_path.push(CONFIG_DIR_NAME);
    config_path.push(CONFIG_FILE_NAME);

    config_path
}

pub fn load_config(config_path: Option<std::path::PathBuf>) -> Config {
    let explicit_path = config_path.is_some();
    let config_path = config_path.unwrap_or_else(default_config_path);

    let config_file = gio::File::for_path(&config_path);

    let data = match config_file.load_bytes(gio::Cancellable::NONE) {
        Err(e) => {
            if explicit_path {
                glib::g_warning!(G_LOG_DOMAIN, "can't read config file: {}", e);
            }
            return Default::default();
        }
        Ok((data, _)) => data,
    };

    let text = match std::str::from_utf8(&data) {
        Err(e) => {
            glib::g_warning!(
                G_LOG_DOMAIN,
                "failed to read config file as utf-8 string: {}",
                e
            );
            return Default::default();
        }
        Ok(text) => text,
    };

    let d = toml::de::Deserializer::new(text);
    match serde_ignored::deserialize(d, |path| {
        glib::g_warning!(G_LOG_DOMAIN, "unknown key in config file: `{}`", path)
    }) {
        Ok(config) => config,
        Err(e) => {
            glib::g_warning!(G_LOG_DOMAIN, "failed to parse config file: {}", e);
            Default::default()
        }
    }
}
