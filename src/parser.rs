use vte::Params;

use crate::terminal::{Rgb, CursorStyle, Terminal};

pub struct Parser {
    terminal: *mut Terminal,
}

impl Parser {
    pub fn new(terminal: *mut Terminal) -> Self {
        Self { terminal }
    }
}

fn get_param(params: &Params, idx: usize) -> i64 {
    params.iter()
        .nth(idx)
        .and_then(|p| p.first())
        .copied()
        .map(|v| v as i64)
        .unwrap_or(0)
}

fn get_params_vec(params: &Params) -> Vec<i64> {
    params.iter().map(|p| p.first().copied().unwrap_or(0) as i64).collect()
}

impl vte::Perform for Parser {
    fn print(&mut self, c: char) {
        let terminal = unsafe { &mut *self.terminal };
        terminal.print_char(c);
    }

    fn execute(&mut self, byte: u8) {
        let terminal = unsafe { &mut *self.terminal };
        match byte {
            0x07 => terminal.bell(),
            0x08 => terminal.backspace(),
            0x09 => terminal.tab(),
            0x0A | 0x0B | 0x0C => terminal.newline(),
            0x0D => terminal.carriage_return(),
            0x0E => {}
            0x0F => {}
            0x7F => {}
            _ => {}
        }
    }

    fn csi_dispatch(&mut self, params: &Params, _intermediates: &[u8], _ignore: bool, action: char) {
        let terminal = unsafe { &mut *self.terminal };
        let p = get_params_vec(params);
        let p0 = p.first().copied().unwrap_or(0);
        let p1 = p.get(1).copied().unwrap_or(0);
        let _p2 = p.get(2).copied().unwrap_or(0);

        match action {
            '@' => terminal.insert_blank(p0),
            'A' => terminal.move_cursor_up(p0),
            'B' => terminal.move_cursor_down(p0),
            'C' => terminal.move_cursor_forward(p0),
            'D' => terminal.move_cursor_backward(p0),
            'E' => {
                terminal.move_cursor_down(p0);
                terminal.carriage_return();
            }
            'F' => {
                terminal.move_cursor_up(p0);
                terminal.carriage_return();
            }
            'G' => terminal.set_cursor_column(p0),
            'H' | 'f' => {
                let row = if p0 == 0 { 1 } else { p0 };
                let col = if p1 == 0 { 1 } else { p1 };
                terminal.set_cursor(col as usize, row as usize);
            }
            'J' => terminal.erase_display(p0),
            'K' => terminal.erase_line(p0),
            'L' => terminal.insert_lines(p0),
            'M' => terminal.delete_lines(p0),
            'P' => terminal.delete_chars(p0),
            'X' => terminal.erase_chars(p0),
            'S' => {
                let count = p0.max(1) as usize;
                for _ in 0..count {
                    terminal.index();
                }
            }
            'T' => {
                let count = p0.max(1) as usize;
                for _ in 0..count {
                    terminal.reverse_index();
                }
            }
            'd' => terminal.set_cursor_row(p0),
            'm' => handle_sgr(terminal, &p),
            'n' => {}
            'r' => {
                let top = p.first().copied().unwrap_or(1);
                let bottom = p.get(1).copied().unwrap_or(terminal.rows() as i64);
                terminal.set_scroll_region(top, bottom);
            }
            'h' => {
                if _intermediates.is_empty() {
                    terminal.set_mode(p0, true);
                }
            }
            'l' => {
                if _intermediates.is_empty() {
                    terminal.set_mode(p0, false);
                }
            }
            's' => terminal.save_cursor(),
            'u' => terminal.restore_cursor(),
            'q' => {
                let style = match p0 {
                    0 | 1 => CursorStyle::BlinkingBlock,
                    2 => CursorStyle::Block,
                    3 => CursorStyle::BlinkingUnderline,
                    4 => CursorStyle::Underline,
                    5 => CursorStyle::BlinkingBar,
                    6 => CursorStyle::Bar,
                    _ => return,
                };
                terminal.set_cursor_style(style);
            }
            't' => {}
            _ => {}
        }
    }

    fn esc_dispatch(&mut self, _intermediates: &[u8], _ignore: bool, byte: u8) {
        let terminal = unsafe { &mut *self.terminal };
        match byte {
            b'7' => terminal.save_cursor(),
            b'8' => terminal.restore_cursor(),
            b'D' => terminal.index(),
            b'E' => terminal.newline(),
            b'H' => terminal.set_tab_stop(),
            b'M' => terminal.reverse_index(),
            b'Z' => {}
            b'c' => {}
            b'g' => {}
            _ => {}
        }
    }

    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        let terminal = unsafe { &mut *self.terminal };
        if params.is_empty() { return; }

        let osc_str = std::str::from_utf8(params[0]).unwrap_or("");
        match osc_str {
            "0" | "2" => {
                if params.len() > 1 {
                    let title = std::str::from_utf8(params[1]).unwrap_or("");
                    terminal.set_title(title);
                }
            }
            "10" | "11" | "12" => {}
            _ => {}
        }
    }

    fn hook(&mut self, _params: &Params, _intermediates: &[u8], _ignore: bool, _action: char) {}

    fn put(&mut self, _byte: u8) {}

    fn unhook(&mut self) {}
}

fn handle_sgr(terminal: &mut Terminal, params: &[i64]) {
    if params.is_empty() || params[0] == 0 {
        terminal.reset_attributes();
        return;
    }

    let mut i = 0;
    while i < params.len() {
        let p = params[i];
        match p {
            0 => terminal.reset_attributes(),
            1 => terminal.set_bold(true),
            2 => terminal.set_bold(false),
            3 => terminal.set_italic(true),
            4 => terminal.set_underline(true),
            5 | 6 => {}
            7 => terminal.set_inverse(true),
            8 => {}
            9 => terminal.set_strikethrough(true),
            21 => terminal.set_bold(false),
            22 => terminal.set_bold(false),
            23 => terminal.set_italic(false),
            24 => terminal.set_underline(false),
            25 => {}
            27 => terminal.set_inverse(false),
            29 => terminal.set_strikethrough(false),
            30..=37 => {
                let idx = (p - 30) as usize;
                let palette = *terminal.color_palette();
                if idx < 16 {
                    let c = palette[idx];
                    terminal.set_fg_color(Rgb::new(c[0], c[1], c[2]));
                }
            }
            38 => {
                if i + 1 < params.len() {
                    match params[i + 1] {
                        2 => {
                            if i + 4 < params.len() {
                                let r = params[i + 2].clamp(0, 255) as u8;
                                let g = params[i + 3].clamp(0, 255) as u8;
                                let b = params[i + 4].clamp(0, 255) as u8;
                                terminal.set_fg_color(Rgb::new(r, g, b));
                                i += 4;
                            }
                        }
                        5 => {
                            if i + 2 < params.len() {
                                let idx = params[i + 2].clamp(0, 255) as u8;
                                terminal.set_fg_color(indexed_color(idx, terminal.color_palette()));
                                i += 2;
                            }
                        }
                        _ => {}
                    }
                }
            }
            39 => {
                terminal.set_fg_color(Rgb::new(0xcc, 0xcc, 0xcc));
            }
            40..=47 => {
                let idx = (p - 40) as usize;
                let palette = *terminal.color_palette();
                if idx < 16 {
                    let c = palette[idx];
                    terminal.set_bg_color(Rgb::new(c[0], c[1], c[2]));
                }
            }
            48 => {
                if i + 1 < params.len() {
                    match params[i + 1] {
                        2 => {
                            if i + 4 < params.len() {
                                let r = params[i + 2].clamp(0, 255) as u8;
                                let g = params[i + 3].clamp(0, 255) as u8;
                                let b = params[i + 4].clamp(0, 255) as u8;
                                terminal.set_bg_color(Rgb::new(r, g, b));
                                i += 4;
                            }
                        }
                        5 => {
                            if i + 2 < params.len() {
                                let idx = params[i + 2].clamp(0, 255) as u8;
                                terminal.set_bg_color(indexed_color(idx, terminal.color_palette()));
                                i += 2;
                            }
                        }
                        _ => {}
                    }
                }
            }
            49 => {
                terminal.set_bg_color(Rgb::new(0, 0, 0));
            }
            90..=97 => {
                let idx = (p - 90 + 8) as usize;
                let palette = *terminal.color_palette();
                if idx < 16 {
                    let c = palette[idx];
                    terminal.set_fg_color(Rgb::new(c[0], c[1], c[2]));
                }
            }
            100..=107 => {
                let idx = (p - 100 + 8) as usize;
                let palette = *terminal.color_palette();
                if idx < 16 {
                    let c = palette[idx];
                    terminal.set_bg_color(Rgb::new(c[0], c[1], c[2]));
                }
            }
            _ => {}
        }
        i += 1;
    }
}

fn indexed_color(idx: u8, palette: &[[u8; 3]; 16]) -> Rgb {
    if idx < 16 {
        Rgb::new(palette[idx as usize][0], palette[idx as usize][1], palette[idx as usize][2])
    } else if idx < 232 {
        let n = idx - 16;
        let r = (n / 36) * 42 + 5;
        let g = ((n / 6) % 6) * 42 + 5;
        let b = (n % 6) * 42 + 5;
        Rgb::new(r, g, b)
    } else {
        let gray = (idx - 232) * 10 + 8;
        Rgb::new(gray, gray, gray)
    }
}
