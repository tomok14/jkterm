use std::collections::VecDeque;

const SCROLLBACK_LINES: usize = 10000;

#[derive(Clone, Copy, Debug)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Cell {
    pub ch: char,
    pub fg: Rgb,
    pub bg: Rgb,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
    pub inverse: bool,
    pub blank: bool,
    pub is_wide: bool,
    pub is_wide_continuation: bool,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            ch: ' ',
            fg: Rgb::new(0, 0, 0),
            bg: Rgb::new(0, 0, 0),
            bold: false,
            italic: false,
            underline: false,
            strikethrough: false,
            inverse: false,
            blank: true,
            is_wide: false,
            is_wide_continuation: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Cursor {
    pub x: usize,
    pub y: usize,
    pub visible: bool,
}

#[derive(Clone, Copy, Debug)]
pub enum CursorStyle {
    Block,
    Underline,
    BlinkingBlock,
    BlinkingUnderline,
    Bar,
    BlinkingBar,
}

impl Default for CursorStyle {
    fn default() -> Self {
        Self::Block
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TerminalModes {
    pub insert: bool,
    pub auto_wrap: bool,
    pub reverse_video: bool,
    pub origin: bool,
    pub application_cursor: bool,
    pub bracketed_paste: bool,
    pub alt_screen: bool,
    pub mouse_tracking: bool,
    pub mouse_motion_tracking: bool,
    pub focus_events: bool,
}

impl Default for TerminalModes {
    fn default() -> Self {
        Self {
            insert: false,
            auto_wrap: true,
            origin: false,
            reverse_video: false,
            application_cursor: false,
            bracketed_paste: false,
            alt_screen: false,
            mouse_tracking: false,
            mouse_motion_tracking: false,
            focus_events: false,
        }
    }
}

pub struct Terminal {
    rows: usize,
    cols: usize,
    grid: Vec<Vec<Cell>>,
    alt_grid: Vec<Vec<Cell>>,
    scrollback: VecDeque<Vec<Cell>>,
    cursor: Cursor,
    saved_cursor: Cursor,
    alt_cursor: Cursor,
    modes: TerminalModes,
    cursor_style: CursorStyle,
    scroll_top: usize,
    scroll_bottom: usize,
    fg_color: Rgb,
    bg_color: Rgb,
    bold: bool,
    italic: bool,
    underline: bool,
    strikethrough: bool,
    inverse: bool,
    foreground: Rgb,
    background: Rgb,
    color_palette: [[u8; 3]; 16],
    tab_stops: Vec<bool>,
    title: String,
    dirty: bool,
}

impl Terminal {
    pub fn new(rows: usize, cols: usize, color_palette: [[u8; 3]; 16]) -> Self {
        let grid = Self::alloc_grid(rows, cols);
        let alt_grid = Self::alloc_grid(rows, cols);
        let tab_stops = (0..cols).map(|i| i % 8 == 0).collect();

        Self {
            rows,
            cols,
            grid,
            alt_grid,
            scrollback: VecDeque::with_capacity(SCROLLBACK_LINES),
            cursor: Cursor::default(),
            saved_cursor: Cursor::default(),
            alt_cursor: Cursor::default(),
            modes: TerminalModes::default(),
            cursor_style: CursorStyle::default(),
            scroll_top: 0,
            scroll_bottom: rows.saturating_sub(1),
            fg_color: Rgb::new(0xcc, 0xcc, 0xcc),
            bg_color: Rgb::new(0, 0, 0),
            bold: false,
            italic: false,
            underline: false,
            strikethrough: false,
            inverse: false,
            foreground: Rgb::new(0xcc, 0xcc, 0xcc),
            background: Rgb::new(0, 0, 0),
            color_palette,
            tab_stops,
            title: String::new(),
            dirty: true,
        }
    }

    fn alloc_grid(rows: usize, cols: usize) -> Vec<Vec<Cell>> {
        vec![vec![Cell::default(); cols]; rows]
    }

    pub fn rows(&self) -> usize { self.rows }
    pub fn cols(&self) -> usize { self.cols }
    pub fn cursor(&self) -> Cursor { self.cursor }
    pub fn cursor_style(&self) -> CursorStyle { self.cursor_style }
    pub fn modes(&self) -> TerminalModes { self.modes }
    pub fn title(&self) -> &str { &self.title }
    pub fn is_dirty(&self) -> bool { self.dirty }

    pub fn grid(&self) -> &Vec<Vec<Cell>> {
        if self.modes.alt_screen { &self.alt_grid } else { &self.grid }
    }

    fn grid_mut(&mut self) -> &mut Vec<Vec<Cell>> {
        self.dirty = true;
        if self.modes.alt_screen { &mut self.alt_grid } else { &mut self.grid }
    }

    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }

    pub fn resize(&mut self, rows: usize, cols: usize) {
        if rows == self.rows && cols == self.cols { return; }

        let old_grid = std::mem::replace(&mut self.grid, Self::alloc_grid(rows, cols));
        let _old_alt = std::mem::replace(&mut self.alt_grid, Self::alloc_grid(rows, cols));

        self.rows = rows;
        self.cols = cols;
        self.scroll_bottom = rows.saturating_sub(1);
        self.cursor.x = self.cursor.x.min(cols.saturating_sub(1));
        self.cursor.y = self.cursor.y.min(rows.saturating_sub(1));

        self.tab_stops = (0..cols).map(|i| i % 8 == 0).collect();

        for (y, row) in old_grid.into_iter().enumerate() {
            if y < rows {
                for (x, cell) in row.into_iter().enumerate() {
                    if x < cols {
                        self.grid[y][x] = cell;
                    }
                }
            }
        }
        self.dirty = true;
    }

    fn cell_at(&self, x: usize, y: usize) -> &Cell {
        &self.grid()[y][x]
    }

    fn cell_at_mut(&mut self, x: usize, y: usize) -> &mut Cell {
        &mut self.grid_mut()[y][x]
    }

    fn is_inside_scroll_region(&self, y: usize) -> bool {
        y >= self.scroll_top && y <= self.scroll_bottom
    }

    fn scroll_up(&mut self, count: usize) {
        let scroll_top = self.scroll_top;
        let scroll_bottom = self.scroll_bottom;
        let cols = self.cols;
        let rows = self.rows;
        let count = count.min(scroll_bottom - scroll_top + 1);
        if count == 0 { return; }

        let removed_rows = {
            let grid = self.grid_mut();
            let mut removed = Vec::with_capacity(count);
            for _ in 0..count {
                removed.push(grid.remove(scroll_top));
            }
            for _ in 0..count {
                grid.insert(scroll_bottom, vec![Cell::default(); cols]);
            }
            grid.truncate(rows);
            removed
        };

        for row in removed_rows {
            if self.scrollback.len() >= SCROLLBACK_LINES {
                self.scrollback.pop_front();
            }
            self.scrollback.push_back(row);
        }
    }

    fn scroll_down(&mut self, count: usize) {
        let scroll_top = self.scroll_top;
        let scroll_bottom = self.scroll_bottom;
        let cols = self.cols;
        let rows = self.rows;
        let count = count.min(scroll_bottom - scroll_top + 1);
        if count == 0 { return; }

        let grid = self.grid_mut();
        for _ in 0..count {
            grid.remove(scroll_bottom);
            grid.insert(scroll_top, vec![Cell::default(); cols]);
        }
        grid.truncate(rows);
    }

    pub fn newline(&mut self) {
        if self.cursor.y >= self.scroll_bottom {
            self.scroll_up(1);
        } else {
            self.cursor.y += 1;
        }
        self.cursor.x = 0;
    }

    pub fn carriage_return(&mut self) {
        self.cursor.x = 0;
    }

    pub fn backspace(&mut self) {
        if self.cursor.x > 0 {
            self.cursor.x -= 1;
        }
    }

    pub fn tab(&mut self) {
        loop {
            if self.cursor.x >= self.cols.saturating_sub(1) {
                break;
            }
            self.cursor.x += 1;
            if self.cursor.x < self.tab_stops.len() && self.tab_stops[self.cursor.x] {
                break;
            }
        }
    }

    pub fn reverse_index(&mut self) {
        if self.cursor.y == self.scroll_top {
            self.scroll_down(1);
        } else if self.cursor.y > 0 {
            self.cursor.y -= 1;
        }
    }

    pub fn print_char(&mut self, c: char) {
        if self.modes.insert {
            let row = self.cursor.y;
            let cols = self.cols;
            let cx = self.cursor.x;
            let grid = self.grid_mut();
            for x in (cx..cols.saturating_sub(1)).rev() {
                grid[row][x + 1] = grid[row][x];
            }
        }

        let fg = self.fg_color;
        let bg = self.bg_color;
        let bold = self.bold;
        let italic = self.italic;
        let underline = self.underline;
        let strikethrough = self.strikethrough;
        let inverse = self.inverse;

        let cell = self.cell_at_mut(self.cursor.x, self.cursor.y);
        *cell = Cell {
            ch: c,
            fg,
            bg,
            bold,
            italic,
            underline,
            strikethrough,
            inverse,
            blank: false,
            is_wide: false,
            is_wide_continuation: false,
        };

        if self.cursor.x + 1 >= self.cols {
            if self.modes.auto_wrap {
                self.cursor.x = 0;
                self.newline();
            }
        } else {
            self.cursor.x += 1;
        }
    }

    pub fn erase_display(&mut self, mode: i64) {
        let rows = self.rows;
        let cols = self.cols;
        let cy = self.cursor.y;
        let cx = self.cursor.x;
        let grid = self.grid_mut();
        match mode {
            0 => {
                for y in cy..rows {
                    let start_x = if y == cy { cx } else { 0 };
                    for x in start_x..cols {
                        grid[y][x] = Cell::default();
                    }
                }
            }
            1 => {
                for y in 0..=cy {
                    let end_x = if y == cy { cx.min(cols.saturating_sub(1)) } else { cols.saturating_sub(1) };
                    for x in 0..=end_x {
                        grid[y][x] = Cell::default();
                    }
                }
            }
            2 | 3 => {
                for row in grid.iter_mut() {
                    for cell in row.iter_mut() {
                        *cell = Cell::default();
                    }
                }
            }
            _ => {}
        }
    }

    pub fn erase_line(&mut self, mode: i64) {
        let cols = self.cols;
        let cx = self.cursor.x;
        let cy = self.cursor.y;
        let grid = self.grid_mut();
        let row = &mut grid[cy];
        match mode {
            0 => {
                for x in cx..cols {
                    row[x] = Cell::default();
                }
            }
            1 => {
                for x in 0..=cx.min(cols.saturating_sub(1)) {
                    row[x] = Cell::default();
                }
            }
            2 => {
                for cell in row.iter_mut() {
                    *cell = Cell::default();
                }
            }
            _ => {}
        }
    }

    pub fn erase_chars(&mut self, count: i64) {
        let count = count.max(1) as usize;
        let cols = self.cols;
        let cx = self.cursor.x;
        let cy = self.cursor.y;
        let grid = self.grid_mut();
        let row = &mut grid[cy];
        let end = (cx + count).min(cols);
        for x in cx..end {
            row[x] = Cell::default();
        }
    }

    pub fn delete_chars(&mut self, count: i64) {
        let count = count.max(1) as usize;
        let cols = self.cols;
        let cx = self.cursor.x;
        let cy = self.cursor.y;
        let grid = self.grid_mut();
        let row = &mut grid[cy];
        for x in cx..cols {
            if x + count < cols {
                row[x] = row[x + count];
            } else {
                row[x] = Cell::default();
            }
        }
    }

    pub fn insert_lines(&mut self, count: i64) {
        let count = count.max(1) as usize;
        if !self.is_inside_scroll_region(self.cursor.y) { return; }
        let scroll_bottom = self.scroll_bottom;
        let cols = self.cols;
        let rows = self.rows;
        let cy = self.cursor.y;
        let region_height = scroll_bottom - cy + 1;
        let count = count.min(region_height);

        let grid = self.grid_mut();
        for _ in 0..count {
            grid.remove(scroll_bottom);
            grid.insert(cy, vec![Cell::default(); cols]);
        }
        grid.truncate(rows);
    }

    pub fn delete_lines(&mut self, count: i64) {
        let count = count.max(1) as usize;
        if !self.is_inside_scroll_region(self.cursor.y) { return; }
        let scroll_bottom = self.scroll_bottom;
        let cols = self.cols;
        let rows = self.rows;
        let cy = self.cursor.y;
        let region_height = scroll_bottom - cy + 1;
        let count = count.min(region_height);

        let grid = self.grid_mut();
        for _ in 0..count {
            grid.remove(cy);
        }
        for _ in 0..count {
            grid.insert(scroll_bottom, vec![Cell::default(); cols]);
        }
        grid.truncate(rows);
    }

    pub fn set_cursor(&mut self, x: usize, y: usize) {
        self.cursor.x = x.min(self.cols.saturating_sub(1));
        if self.modes.origin {
            self.cursor.y = (self.scroll_top + y).min(self.scroll_bottom);
        } else {
            self.cursor.y = y.min(self.rows.saturating_sub(1));
        }
    }

    pub fn set_cursor_column(&mut self, x: i64) {
        let x = x.max(1) as usize;
        self.cursor.x = x.saturating_sub(1).min(self.cols.saturating_sub(1));
    }

    pub fn set_cursor_row(&mut self, y: i64) {
        let y = y.max(1) as usize;
        if self.modes.origin {
            self.cursor.y = (self.scroll_top + y - 1).min(self.scroll_bottom);
        } else {
            self.cursor.y = (y - 1).min(self.rows.saturating_sub(1));
        }
    }

    pub fn move_cursor_up(&mut self, count: i64) {
        let count = count.max(1) as usize;
        if self.modes.origin {
            self.cursor.y = self.cursor.y.saturating_sub(count).max(self.scroll_top);
        } else {
            self.cursor.y = self.cursor.y.saturating_sub(count);
        }
    }

    pub fn move_cursor_down(&mut self, count: i64) {
        let count = count.max(1) as usize;
        if self.modes.origin {
            self.cursor.y = (self.cursor.y + count).min(self.scroll_bottom);
        } else {
            self.cursor.y = (self.cursor.y + count).min(self.rows.saturating_sub(1));
        }
    }

    pub fn move_cursor_forward(&mut self, count: i64) {
        let count = count.max(1) as usize;
        self.cursor.x = (self.cursor.x + count).min(self.cols.saturating_sub(1));
    }

    pub fn move_cursor_backward(&mut self, count: i64) {
        let count = count.max(1) as usize;
        self.cursor.x = self.cursor.x.saturating_sub(count);
    }

    pub fn set_scroll_region(&mut self, top: i64, bottom: i64) {
        self.scroll_top = (top.max(1) as usize).saturating_sub(1).min(self.rows.saturating_sub(1));
        self.scroll_bottom = (bottom.max(1) as usize).saturating_sub(1).min(self.rows.saturating_sub(1));
        if self.scroll_bottom < self.scroll_top {
            std::mem::swap(&mut self.scroll_top, &mut self.scroll_bottom);
        }
    }

    pub fn save_cursor(&mut self) {
        let cursor = Cursor { x: self.cursor.x, y: self.cursor.y, visible: self.cursor.visible };
        if self.modes.alt_screen {
            self.alt_cursor = cursor;
        } else {
            self.saved_cursor = cursor;
        }
    }

    pub fn restore_cursor(&mut self) {
        let saved = if self.modes.alt_screen { self.alt_cursor } else { self.saved_cursor };
        self.cursor = saved;
        self.cursor.x = self.cursor.x.min(self.cols.saturating_sub(1));
        self.cursor.y = self.cursor.y.min(self.rows.saturating_sub(1));
    }

    pub fn set_mode(&mut self, mode: i64, set: bool) {
        match mode {
            4 => self.modes.insert = set,
            7 => self.modes.auto_wrap = set,
            12 => self.cursor_style = if set { CursorStyle::BlinkingBlock } else { CursorStyle::Block },
            20 => self.modes.application_cursor = set,
            25 => self.cursor.visible = set,
            47 | 1047 | 1049 => {
                if set && !self.modes.alt_screen {
                    self.switch_to_alt_screen();
                } else if !set && self.modes.alt_screen {
                    self.switch_to_primary_screen();
                }
            }
            1000 => self.modes.mouse_tracking = set,
            1002 => self.modes.mouse_motion_tracking = set,
            1004 => self.modes.focus_events = set,
            2004 => self.modes.bracketed_paste = set,
            _ => {}
        }
    }

    fn switch_to_alt_screen(&mut self) {
        self.alt_cursor = Cursor { x: self.cursor.x, y: self.cursor.y, visible: self.cursor.visible };
        self.cursor = Cursor::default();
        self.modes.alt_screen = true;
    }

    fn switch_to_primary_screen(&mut self) {
        self.modes.alt_screen = false;
        self.cursor = self.alt_cursor;
        self.cursor.x = self.cursor.x.min(self.cols.saturating_sub(1));
        self.cursor.y = self.cursor.y.min(self.rows.saturating_sub(1));
    }

    pub fn set_cursor_style(&mut self, style: CursorStyle) {
        self.cursor_style = style;
    }

    pub fn set_fg_color(&mut self, color: Rgb) {
        self.fg_color = color;
    }

    pub fn set_bg_color(&mut self, color: Rgb) {
        self.bg_color = color;
    }

    pub fn set_bold(&mut self, bold: bool) {
        self.bold = bold;
    }

    pub fn set_italic(&mut self, italic: bool) {
        self.italic = italic;
    }

    pub fn set_underline(&mut self, underline: bool) {
        self.underline = underline;
    }

    pub fn set_strikethrough(&mut self, strikethrough: bool) {
        self.strikethrough = strikethrough;
    }

    pub fn set_inverse(&mut self, inverse: bool) {
        self.inverse = inverse;
    }

    pub fn reset_attributes(&mut self) {
        self.fg_color = self.foreground;
        self.bg_color = self.background;
        self.bold = false;
        self.italic = false;
        self.underline = false;
        self.strikethrough = false;
        self.inverse = false;
    }

    pub fn set_title(&mut self, title: &str) {
        self.title = title.to_string();
    }

    pub fn color_palette(&self) -> &[[u8; 3]; 16] {
        &self.color_palette
    }

    pub fn index(&mut self) {
        if self.cursor.y == self.scroll_bottom {
            self.scroll_up(1);
        } else {
            self.cursor.y = (self.cursor.y + 1).min(self.rows.saturating_sub(1));
        }
    }

    pub fn bell(&self) {
    }

    pub fn set_tab_stop(&mut self) {
        if self.cursor.x < self.tab_stops.len() {
            self.tab_stops[self.cursor.x] = true;
        }
    }

    pub fn clear_tab_stop(&mut self, mode: i64) {
        match mode {
            0 if self.cursor.x < self.tab_stops.len() => self.tab_stops[self.cursor.x] = false,
            3 => self.tab_stops.iter_mut().for_each(|t| *t = false),
            _ => {}
        }
    }

    pub fn insert_blank(&mut self, count: i64) {
        let count = count.max(1) as usize;
        let cols = self.cols;
        let cx = self.cursor.x;
        let cy = self.cursor.y;
        let grid = self.grid_mut();
        let row = &mut grid[cy];
        let start = cx;
        let end = (cols - count).min(cols);
        if start < end {
            for x in (start..end).rev() {
                row[x + count] = row[x];
            }
        }
        for x in start..(start + count).min(cols) {
            row[x] = Cell::default();
        }
    }
}
