pub struct Config {
    pub rows: u16,
    pub cols: u16,
    pub font_size: f32,
    pub font_family: String,
    pub shell: String,
    pub background: [f32; 4],
    pub foreground: [f32; 4],
    pub padding_x: f64,
    pub padding_y: f64,
    pub cursor_color: [f32; 4],
    pub selection_color: [f32; 4],
    pub color_palette: [[u8; 3]; 16],
}

impl Default for Config {
    fn default() -> Self {
        Self {
            rows: 24,
            cols: 80,
            font_size: 14.0,
            font_family: "JetBrains Mono, Fira Code, Iosevka, monospace".into(),
            shell: std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into()),
            background: [0.04, 0.04, 0.06, 1.0],
            foreground: [0.93, 0.93, 0.95, 1.0],
            padding_x: 8.0,
            padding_y: 8.0,
            cursor_color: [0.93, 0.93, 0.95, 1.0],
            selection_color: [0.3, 0.3, 0.5, 0.3],
            color_palette: [
                [0x1d, 0x1f, 0x21], // black
                [0xcc, 0x66, 0x66], // red
                [0xb5, 0xbd, 0x68], // green
                [0xf0, 0xc6, 0x74], // yellow
                [0x81, 0xa2, 0xbe], // blue
                [0xb2, 0x94, 0xbb], // magenta
                [0x8a, 0xbe, 0xb7], // cyan
                [0xc5, 0xc8, 0xc6], // white
                [0x66, 0x66, 0x66], // bright black
                [0xd5, 0x4e, 0x53], // bright red
                [0xb9, 0xca, 0x4a], // bright green
                [0xe7, 0xc5, 0x47], // bright yellow
                [0x7a, 0xa6, 0xda], // bright blue
                [0xc3, 0x97, 0xd8], // bright magenta
                [0x70, 0xc0, 0xb1], // bright cyan
                [0xdb, 0xde, 0xdc], // bright white
            ],
        }
    }
}
