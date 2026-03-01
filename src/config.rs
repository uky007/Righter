pub struct Config {
    pub tab_width: usize,
    pub scroll_off: usize,
    pub wrap: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            tab_width: 4,
            scroll_off: 5,
            wrap: false,
        }
    }
}
