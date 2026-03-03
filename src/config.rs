use serde::Deserialize;

pub struct Config {
    pub tab_width: usize,
    pub scroll_off: usize,
    pub wrap: bool,
    pub gui_font_size: f32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            tab_width: 4,
            scroll_off: 5,
            wrap: false,
            gui_font_size: 14.0,
        }
    }
}

pub struct ConfigLoadResult {
    pub config: Config,
    pub warning: Option<String>,
}

#[derive(Deserialize)]
struct ConfigFile {
    tab_width: Option<usize>,
    scroll_off: Option<usize>,
    wrap: Option<bool>,
    font_size: Option<f32>,
}

impl Config {
    pub fn load() -> ConfigLoadResult {
        let config_dir = std::env::var("XDG_CONFIG_HOME")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| {
                let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
                std::path::PathBuf::from(home).join(".config")
            });
        let path = config_dir.join("righter").join("config.json");
        Self::load_from_path(&path)
    }

    pub fn load_from_path(path: &std::path::Path) -> ConfigLoadResult {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound {
                    return ConfigLoadResult {
                        config: Config::default(),
                        warning: None,
                    };
                }
                return ConfigLoadResult {
                    config: Config::default(),
                    warning: Some(format!("Config: {e}")),
                };
            }
        };

        let file: ConfigFile = match serde_json::from_str(&content) {
            Ok(f) => f,
            Err(e) => {
                return ConfigLoadResult {
                    config: Config::default(),
                    warning: Some(format!("Config parse error: {e}")),
                };
            }
        };

        let defaults = Config::default();
        ConfigLoadResult {
            config: Config {
                tab_width: file.tab_width.unwrap_or(defaults.tab_width),
                scroll_off: file.scroll_off.unwrap_or(defaults.scroll_off),
                wrap: file.wrap.unwrap_or(defaults.wrap),
                gui_font_size: file.font_size.unwrap_or(defaults.gui_font_size),
            },
            warning: None,
        }
    }
}
