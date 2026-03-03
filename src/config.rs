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
