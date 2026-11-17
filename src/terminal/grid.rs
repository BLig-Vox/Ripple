use std::collections::VecDeque;

use super::graphics::{self, Graphic, SixelInput};
use crate::keyboard::KeyboardMode;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use vte::{Params, Perform};

const DEFAULT_FOREGROUND: Rgb = Rgb(216, 216, 216);
pub const DEFAULT_BACKGROUND: Rgb = Rgb(24, 24, 24);
const SCROLLBACK_LIMIT: usize = 2_000;
const SCROLLBACK_BYTES: usize = 8 * 1024 * 1024;
const MAX_COLUMNS: usize = 512;
const MAX_ROWS: usize = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Rgb(pub u8, pub u8, pub u8);

#[derive(Clone, Copy, Debug)]
pub struct Cell {
    pub character: char,
    pub foreground: Rgb,
    pub background: Rgb,
    pub link: u32,
}

impl Cell {
    fn blank() -> Self {
        Self {
            character: ' ',
            foreground: DEFAULT_FOREGROUND,
            background: DEFAULT_BACKGROUND,
            link: 0,
        }
    }
}

struct SavedScreen {
    images: VecDeque<Graphic>,
    link: u32,
    cells: Vec<Vec<Cell>>,
    cursor: (usize, usize),
    foreground: Rgb,
    background: Rgb,
    scroll_top: usize,
    scroll_bottom: usize,
}

pub struct TerminalGrid {
    rows: usize,
    columns: usize,
    cursor_row: usize,
    cursor_column: usize,
    foreground: Rgb,
    background: Rgb,
    pub cells: Vec<Vec<Cell>>,
    scrollback: VecDeque<Vec<Cell>>,
    scrollback_bytes: usize,
    pub parser: vte::Parser<4096>,
    pub images: VecDeque<Graphic>,
    cell_pixels: (usize, usize),
    image_id: u64,
    sixel: Option<SixelInput>,
    pending_sixel: Option<SixelInput>,
    sixel_scroll: bool,
    sixel_right: bool,
    links: HashMap<u32, Arc<str>>,
    current_link: u32,
    link_id: u32,
    primary_screen: Option<SavedScreen>,
    scroll_top: usize,
    scroll_bottom: usize,
    saved_cursor: (usize, usize),
    pub cursor_visible: bool,
    pub application_cursor: bool,
    pub mouse_tracking: bool,
    pub sgr_mouse: bool,
    pub bracketed_paste: bool,
    auto_wrap: bool,
    inverse: bool,
    replies: Vec<u8>,
    primary_keyboard: KeyboardMode,
    alternate_keyboard: KeyboardMode,
}

impl TerminalGrid {
    pub fn new(columns: usize, rows: usize) -> Self {
        let columns = columns.clamp(1, MAX_COLUMNS);
        let rows = rows.clamp(1, MAX_ROWS);
        Self {
            rows,
            columns,
            cursor_row: 0,
            cursor_column: 0,
            foreground: DEFAULT_FOREGROUND,
            background: DEFAULT_BACKGROUND,
            cells: vec![vec![Cell::blank(); columns]; rows],
            scrollback: VecDeque::new(),
            scrollback_bytes: 0,
            parser: vte::Parser::<4096>::new_with_size(),
            images: VecDeque::new(),
            cell_pixels: (9, 19),
            image_id: 0,
            sixel: None,
            pending_sixel: None,
            sixel_scroll: true,
            sixel_right: false,
            links: HashMap::new(),
            current_link: 0,
            link_id: 0,
            primary_screen: None,
            scroll_top: 0,
            scroll_bottom: rows - 1,
            saved_cursor: (0, 0),
            cursor_visible: true,
            application_cursor: false,
            mouse_tracking: false,
            sgr_mouse: false,
            bracketed_paste: false,
            auto_wrap: true,
            inverse: false,
            replies: Vec::new(),
            primary_keyboard: KeyboardMode::default(),
            alternate_keyboard: KeyboardMode::default(),
        }
    }

    pub fn dimensions(&self) -> (usize, usize) {
        (self.columns, self.rows)
    }

    pub fn cursor(&self) -> (usize, usize) {
        (self.cursor_column.min(self.columns - 1), self.cursor_row)
    }

    pub fn scrollback_len(&self) -> usize {
        if self.alternate_screen_active() {
            0
        } else {
            self.scrollback.len()
        }
    }

    pub fn alternate_screen_active(&self) -> bool {
        self.primary_screen.is_some()
    }

    pub fn keyboard_flags(&self) -> u8 {
        if self.alternate_screen_active() {
            self.alternate_keyboard.flags()
        } else {
            self.primary_keyboard.flags()
        }
    }

    fn keyboard_mode_mut(&mut self) -> &mut KeyboardMode {
        if self.alternate_screen_active() {
            &mut self.alternate_keyboard
        } else {
            &mut self.primary_keyboard
        }
    }

    pub fn take_replies(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.replies)
    }

    fn reply(&mut self, bytes: &[u8]) {
        if self.replies.len() + bytes.len() <= 4096 {
            self.replies.extend_from_slice(bytes);
        }
    }

    fn switch_alternate_screen(&mut self, enabled: bool) {
        if enabled && self.primary_screen.is_none() {
            self.alternate_keyboard = KeyboardMode::default();
            let empty = vec![self.blank_line(); self.rows];
            self.primary_screen = Some(SavedScreen {
                images: std::mem::take(&mut self.images),
                link: self.current_link,
                cells: std::mem::replace(&mut self.cells, empty),
                cursor: (self.cursor_column, self.cursor_row),
                foreground: self.foreground,
                background: self.background,
                scroll_top: self.scroll_top,
                scroll_bottom: self.scroll_bottom,
            });
            self.current_link = 0;
            self.cursor_column = 0;
            self.cursor_row = 0;
            self.scroll_top = 0;
            self.scroll_bottom = self.rows - 1;
        } else if !enabled {
            self.alternate_keyboard = KeyboardMode::default();
            if let Some(mut primary) = self.primary_screen.take() {
                for line in &mut primary.cells {
                    line.resize(self.columns, Cell::blank());
                }
                primary.cells.resize(self.rows, self.blank_line());
                primary.images.retain(|image| {
                    image.row + image.rows as i32 <= self.rows as i32
                        && image.column + image.columns <= self.columns
                });
                self.images = primary.images;
                self.current_link = primary.link;
                self.cells = primary.cells;
                self.cursor_column = primary.cursor.0.min(self.columns - 1);
                self.cursor_row = primary.cursor.1.min(self.rows - 1);
                self.foreground = primary.foreground;
                self.background = primary.background;
                self.scroll_top = primary.scroll_top.min(self.rows - 1);
                self.scroll_bottom = primary
                    .scroll_bottom
                    .min(self.rows - 1)
                    .max(self.scroll_top);
            }
        }
    }

    /// Return the row at a visible viewport position, where offset 0 is live output.
    pub fn visible_line(&self, row: usize, scroll_offset: usize) -> &[Cell] {
        if self.alternate_screen_active() {
            return &self.cells[row.min(self.rows - 1)];
        }
        let offset = scroll_offset.min(self.scrollback.len());
        let total_rows = self.scrollback.len() + self.rows;
        let end = total_rows.saturating_sub(offset);
        let start = end.saturating_sub(self.rows);
        let absolute_row = start + row.min(self.rows - 1);
        if absolute_row < self.scrollback.len() {
            &self.scrollback[absolute_row]
        } else {
            &self.cells[absolute_row - self.scrollback.len()]
        }
    }

    pub fn set_cell_pixels(&mut self, width: usize, height: usize) {
        self.cell_pixels = (width.clamp(1, 256), height.clamp(1, 256));
    }

    fn erase_images(&mut self, top: i32, bottom: i32, left: usize, right: usize) {
        self.images
            .retain(|image| !image.overlaps(top, bottom, left, right));
    }

    fn finish_sixel(&mut self) {
        let Some(input) = self.pending_sixel.take() else {
            return;
        };
        let Rgb(r, g, b) = self.background;
        let Some(bitmap) = graphics::decode(input, [r, g, b, 255]) else {
            return;
        };
        let rows = bitmap.height.div_ceil(self.cell_pixels.1);
        let columns = bitmap.width.div_ceil(self.cell_pixels.0);
        let (column, row) = if self.sixel_scroll {
            (self.cursor_column.min(self.columns - 1), self.cursor_row)
        } else {
            (0, 0)
        };
        self.erase_images(row as i32, (row + rows) as i32, column, column + columns);
        let blank = self.erase_cell();
        for line in self.cells.iter_mut().skip(row).take(rows) {
            for cell in line.iter_mut().skip(column).take(columns) {
                *cell = blank;
            }
        }
        while self.images.len() >= graphics::IMAGE_COUNT
            || self.images.iter().map(|i| i.pixels.len()).sum::<usize>() + bitmap.pixels.len()
                > graphics::IMAGE_BYTES
        {
            if self.images.pop_front().is_none() {
                return;
            }
        }
        self.image_id = self.image_id.wrapping_add(1);
        self.images.push_back(Graphic {
            id: self.image_id,
            row: row as i32,
            column,
            width: bitmap.width,
            height: bitmap.height,
            rows,
            columns,
            pixels: bitmap.pixels.into(),
        });
        if self.sixel_scroll {
            if self.sixel_right {
                self.cursor_column = (column + columns).min(self.columns - 1);
            } else {
                for _ in 0..rows {
                    self.line_feed();
                }
                self.cursor_column = column;
            }
        }
    }

    pub fn link_at(&self, row: usize, column: usize, offset: usize) -> Option<String> {
        let line = self.visible_line(row, offset);
        let column = column.min(line.len().saturating_sub(1));
        if let Some(link) = self.links.get(&line.get(column)?.link) {
            return Some(link.to_string());
        }
        let mut start = column;
        let mut end = column + 1;
        while start > 0 && !line[start - 1].character.is_whitespace() {
            start -= 1;
        }
        while end < line.len() && !line[end].character.is_whitespace() {
            end += 1;
        }
        // Without soft-wrap metadata, do not open a possibly truncated URL.
        if end == line.len() {
            return None;
        }
        let text: String = line[start..end].iter().map(|cell| cell.character).collect();
        let mut text = text
            .trim_start_matches(['(', '[', '{', '<', '\'', '"'])
            .trim_end_matches(['.', ',', ';', '!', '?', '>', '\'', '"'])
            .to_string();
        for (open, close) in [('(', ')'), ('[', ']'), ('{', '}')] {
            while text.ends_with(close)
                && text.chars().filter(|c| *c == close).count()
                    > text.chars().filter(|c| *c == open).count()
            {
                text.pop();
            }
        }
        crate::links::safe_url(&text).then_some(text)
    }

    fn set_link(&mut self, url: &str) {
        self.current_link = 0;
        if !crate::links::safe_url(url) {
            return;
        }
        if let Some((&id, _)) = self.links.iter().find(|(_, old)| old.as_ref() == url) {
            self.current_link = id;
            return;
        }
        if self.links.len() >= 1024 {
            let mut used = HashSet::new();
            for line in self.cells.iter().chain(self.scrollback.iter()) {
                used.extend(line.iter().map(|cell| cell.link));
            }
            if let Some(primary) = &self.primary_screen {
                used.insert(primary.link);
                for line in &primary.cells {
                    used.extend(line.iter().map(|cell| cell.link));
                }
            }
            self.links.retain(|id, _| used.contains(id));
        }
        if self.links.len() >= 1024 {
            return;
        }
        self.link_id = self.link_id.wrapping_add(1).max(1);
        while self.links.contains_key(&self.link_id) {
            self.link_id = self.link_id.wrapping_add(1).max(1);
        }
        self.current_link = self.link_id;
        self.links.insert(self.link_id, Arc::from(url));
    }

    pub fn feed(&mut self, bytes: &[u8]) {
        let mut parser = std::mem::replace(&mut self.parser, vte::Parser::<4096>::new_with_size());
        parser.advance(self, bytes);
        self.parser = parser;
    }

    pub fn resize(&mut self, columns: usize, rows: usize) {
        let columns = columns.clamp(1, MAX_COLUMNS);
        let rows = rows.clamp(1, MAX_ROWS);
        if columns == self.columns && rows == self.rows {
            return;
        }

        self.images.retain(|image| {
            image.row + image.rows as i32 <= rows as i32 && image.column + image.columns <= columns
        });
        if let Some(primary) = &mut self.primary_screen {
            primary.images.retain(|image| {
                image.row + image.rows as i32 <= rows as i32
                    && image.column + image.columns <= columns
            });
        }
        let blank = self.erase_cell();
        for row in &mut self.cells {
            row.resize(columns, blank);
        }
        self.cells.resize(rows, vec![blank; columns]);
        self.columns = columns;
        self.rows = rows;
        self.cursor_column = self.cursor_column.min(columns - 1);
        self.cursor_row = self.cursor_row.min(rows - 1);
        self.scroll_top = 0;
        self.scroll_bottom = rows - 1;
    }

    // Erase operations use the current background (xterm's BCE capability).
    fn erase_cell(&self) -> Cell {
        Cell {
            character: ' ',
            foreground: self.foreground,
            background: self.background,
            link: 0,
        }
    }

    fn blank_line(&self) -> Vec<Cell> {
        vec![self.erase_cell(); self.columns]
    }

    fn scroll_up(&mut self, count: usize) {
        for _ in 0..count.min(self.scroll_bottom - self.scroll_top + 1) {
            let removed = self.cells.remove(self.scroll_top);
            if self.scroll_top == 0
                && self.scroll_bottom == self.rows - 1
                && !self.alternate_screen_active()
            {
                self.scrollback_bytes += removed.capacity() * std::mem::size_of::<Cell>();
                self.scrollback.push_back(removed);
                while self.scrollback.len() > SCROLLBACK_LIMIT
                    || self.scrollback_bytes > SCROLLBACK_BYTES
                {
                    if let Some(line) = self.scrollback.pop_front() {
                        self.scrollback_bytes -= line.capacity() * std::mem::size_of::<Cell>();
                    }
                }
            }
            self.cells.insert(self.scroll_bottom, self.blank_line());
            if self.scroll_top == 0 && self.scroll_bottom == self.rows - 1 {
                for image in &mut self.images {
                    image.row -= 1;
                }
                let oldest = -(self.scrollback_len() as i32);
                self.images
                    .retain(|image| image.row + image.rows as i32 > oldest);
            } else {
                self.erase_images(
                    self.scroll_top as i32,
                    self.scroll_bottom as i32 + 1,
                    0,
                    self.columns,
                );
            }
        }
    }

    fn scroll_down(&mut self, count: usize) {
        for _ in 0..count.min(self.scroll_bottom - self.scroll_top + 1) {
            self.cells.remove(self.scroll_bottom);
            self.cells.insert(self.scroll_top, self.blank_line());
            if self.scroll_top == 0 && self.scroll_bottom == self.rows - 1 {
                self.images
                    .retain(|image| !(image.row < 0 && image.row + image.rows as i32 > 0));
                for image in &mut self.images {
                    if image.row >= 0 {
                        image.row += 1;
                    }
                }
                self.images.retain(|image| image.row < self.rows as i32);
            } else {
                self.erase_images(
                    self.scroll_top as i32,
                    self.scroll_bottom as i32 + 1,
                    0,
                    self.columns,
                );
            }
        }
    }

    fn line_feed(&mut self) {
        if self.cursor_row == self.scroll_bottom {
            self.scroll_up(1);
        } else {
            self.cursor_row = (self.cursor_row + 1).min(self.rows - 1);
        }
    }

    fn erase_line(&mut self, start: usize, end: usize) {
        self.erase_images(
            self.cursor_row as i32,
            self.cursor_row as i32 + 1,
            start,
            end,
        );
        let blank = self.erase_cell();
        let row = &mut self.cells[self.cursor_row];
        for cell in row
            .iter_mut()
            .take(end.min(self.columns))
            .skip(start.min(self.columns))
        {
            *cell = blank;
        }
    }

    fn erase_display(&mut self, mode: u16) {
        match mode {
            0 => self.erase_images(
                self.cursor_row as i32 + 1,
                self.rows as i32,
                0,
                self.columns,
            ),
            1 => self.erase_images(0, self.cursor_row as i32, 0, self.columns),
            2 => self.erase_images(0, self.rows as i32, 0, self.columns),
            3 => self.images.clear(),
            _ => {}
        }
        let blank = self.erase_cell();
        match mode {
            0 => {
                self.erase_line(self.cursor_column, self.columns);
                for row in self.cursor_row + 1..self.rows {
                    self.cells[row].fill(blank);
                }
            }
            1 => {
                for row in 0..self.cursor_row {
                    self.cells[row].fill(blank);
                }
                self.erase_line(0, self.cursor_column + 1);
            }
            2 | 3 => {
                for row in &mut self.cells {
                    row.fill(blank);
                }
                if mode == 3 {
                    self.scrollback.clear();
                    self.scrollback_bytes = 0;
                }
            }
            _ => {}
        }
    }

    fn set_sgr(&mut self, params: &[u16]) {
        let params = if params.is_empty() { &[0][..] } else { params };
        let mut index = 0;
        while index < params.len() {
            match params[index] {
                0 => {
                    self.inverse = false;
                    self.foreground = DEFAULT_FOREGROUND;
                    self.background = DEFAULT_BACKGROUND;
                }
                30..=37 => self.foreground = ansi_color((params[index] - 30) as usize),
                40..=47 => self.background = ansi_color((params[index] - 40) as usize),
                90..=97 => self.foreground = ansi_color((params[index] - 90 + 8) as usize),
                100..=107 => self.background = ansi_color((params[index] - 100 + 8) as usize),
                7 => self.inverse = true,
                27 => self.inverse = false,
                39 => self.foreground = DEFAULT_FOREGROUND,
                49 => self.background = DEFAULT_BACKGROUND,
                38 | 48 if params.get(index + 1) == Some(&2) && index + 4 < params.len() => {
                    let rgb = Rgb(
                        params[index + 2].min(255) as u8,
                        params[index + 3].min(255) as u8,
                        params[index + 4].min(255) as u8,
                    );
                    if params[index] == 38 {
                        self.foreground = rgb;
                    } else {
                        self.background = rgb;
                    }
                    index += 4;
                }
                38 | 48 if params.get(index + 1) == Some(&5) => {
                    if let Some(color) = params.get(index + 2) {
                        let rgb = indexed_color(*color as usize);
                        if params[index] == 38 {
                            self.foreground = rgb;
                        } else {
                            self.background = rgb;
                        }
                        index += 2;
                    }
                }
                _ => {}
            }
            index += 1;
        }
    }

    fn write_character(&mut self, character: char) {
        if self.cursor_column >= self.columns {
            if self.auto_wrap {
                self.cursor_column = 0;
                self.line_feed();
            } else {
                self.cursor_column = self.columns - 1;
            }
        }
        self.erase_images(
            self.cursor_row as i32,
            self.cursor_row as i32 + 1,
            self.cursor_column,
            self.cursor_column + 1,
        );
        self.cells[self.cursor_row][self.cursor_column] = Cell {
            link: self.current_link,
            character,
            foreground: if self.inverse {
                self.background
            } else {
                self.foreground
            },
            background: if self.inverse {
                self.foreground
            } else {
                self.background
            },
        };
        self.cursor_column += 1;
    }
}

impl Perform for TerminalGrid {
    fn hook(&mut self, params: &Params, intermediates: &[u8], ignore: bool, action: char) {
        self.pending_sixel = None;
        self.sixel = if !ignore && intermediates.is_empty() && action == 'q' {
            let transparent = params.iter().nth(1).and_then(|p| p.first()).copied() == Some(1);
            Some(SixelInput {
                bytes: Vec::new(),
                transparent,
                rejected: false,
            })
        } else {
            None
        };
    }
    fn put(&mut self, byte: u8) {
        if let Some(input) = &mut self.sixel {
            input.put(byte);
        }
    }
    fn unhook(&mut self) {
        self.pending_sixel = self.sixel.take();
    }
    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        self.pending_sixel = None;
        if params.first() != Some(&b"8".as_slice()) || params.len() < 3 {
            return;
        }
        self.current_link = 0;
        let length = params[2..].iter().map(|p| p.len()).sum::<usize>() + params.len() - 3;
        if length > 2048 {
            return;
        }
        let bytes = params[2..].join(&b';');
        if let Ok(url) = std::str::from_utf8(&bytes) {
            self.set_link(url);
        }
    }

    fn print(&mut self, character: char) {
        self.pending_sixel = None;
        self.write_character(character);
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            0x18 | 0x1a => {
                self.sixel = None;
                self.pending_sixel = None;
            }
            b'\n' | 0x0b | 0x0c => self.line_feed(),
            b'\r' => self.cursor_column = 0,
            b'\x08' => self.cursor_column = self.cursor_column.saturating_sub(1),
            b'\t' => {
                let next_tab = ((self.cursor_column / 8) + 1) * 8;
                self.cursor_column = next_tab.min(self.columns - 1);
            }
            _ => {}
        }
    }

    fn csi_dispatch(&mut self, params: &Params, intermediates: &[u8], ignore: bool, action: char) {
        self.pending_sixel = None;
        if ignore {
            return;
        }
        let values: Vec<u16> = params
            .iter()
            .map(|param| param.first().copied().unwrap_or(0))
            .collect();
        if action == 'u' {
            let flags = values.first().copied().unwrap_or(0);
            match intermediates {
                b"?" => self.reply(format!("\x1b[?{}u", self.keyboard_flags()).as_bytes()),
                b">" => self.keyboard_mode_mut().push(flags),
                b"<" => self.keyboard_mode_mut().pop(flags),
                b"=" => self
                    .keyboard_mode_mut()
                    .set(flags, values.get(1).copied().unwrap_or(1)),
                _ => {}
            }
            return;
        }
        if intermediates == b"?" && matches!(action, 'h' | 'l') {
            let enabled = action == 'h';
            for &mode in &values {
                match mode {
                    47 | 1047 | 1049 => self.switch_alternate_screen(enabled),
                    1048 => {
                        if enabled {
                            self.saved_cursor = (self.cursor_column, self.cursor_row);
                        } else {
                            self.cursor_column = self.saved_cursor.0.min(self.columns - 1);
                            self.cursor_row = self.saved_cursor.1.min(self.rows - 1);
                        }
                    }
                    1 => self.application_cursor = enabled,
                    7 => self.auto_wrap = enabled,
                    25 => self.cursor_visible = enabled,
                    1000 | 1002 | 1003 => self.mouse_tracking = enabled,
                    1006 => self.sgr_mouse = enabled,
                    2004 => self.bracketed_paste = enabled,
                    80 => self.sixel_scroll = !enabled,
                    8452 => self.sixel_right = enabled,
                    _ => {}
                }
            }
            return;
        }
        if !intermediates.is_empty() {
            return;
        }
        let parameter = |index: usize, default: usize| -> usize {
            values
                .get(index)
                .copied()
                .filter(|value| *value != 0)
                .map(usize::from)
                .unwrap_or(default)
        };

        match action {
            'A' => self.cursor_row = self.cursor_row.saturating_sub(parameter(0, 1)),
            'B' => self.cursor_row = (self.cursor_row + parameter(0, 1)).min(self.rows - 1),
            'C' => {
                self.cursor_column = (self.cursor_column + parameter(0, 1)).min(self.columns - 1)
            }
            'D' => self.cursor_column = self.cursor_column.saturating_sub(parameter(0, 1)),
            'E' => {
                self.cursor_row = (self.cursor_row + parameter(0, 1)).min(self.rows - 1);
                self.cursor_column = 0;
            }
            'F' => {
                self.cursor_row = self.cursor_row.saturating_sub(parameter(0, 1));
                self.cursor_column = 0;
            }
            'G' | '`' => {
                self.cursor_column = parameter(0, 1).saturating_sub(1).min(self.columns - 1)
            }
            'H' | 'f' => {
                self.cursor_row = parameter(0, 1).saturating_sub(1).min(self.rows - 1);
                self.cursor_column = parameter(1, 1).saturating_sub(1).min(self.columns - 1);
            }
            'J' => self.erase_display(values.first().copied().unwrap_or(0)),
            'K' => match values.first().copied().unwrap_or(0) {
                0 => self.erase_line(self.cursor_column, self.columns),
                1 => self.erase_line(0, self.cursor_column + 1),
                2 => self.erase_line(0, self.columns),
                _ => {}
            },
            'r' => {
                let top = parameter(0, 1).saturating_sub(1).min(self.rows - 1);
                let bottom = parameter(1, self.rows).saturating_sub(1).min(self.rows - 1);
                if top < bottom {
                    self.scroll_top = top;
                    self.scroll_bottom = bottom;
                    self.cursor_row = 0;
                    self.cursor_column = 0;
                }
            }
            'S' => self.scroll_up(parameter(0, 1)),
            'T' => self.scroll_down(parameter(0, 1)),
            'L' | 'M'
                if self.cursor_row >= self.scroll_top && self.cursor_row <= self.scroll_bottom =>
            {
                let top = self.scroll_top;
                self.scroll_top = self.cursor_row;
                if action == 'L' {
                    self.scroll_down(parameter(0, 1));
                } else {
                    self.scroll_up(parameter(0, 1));
                }
                self.scroll_top = top;
            }
            'X' => self.erase_line(self.cursor_column, self.cursor_column + parameter(0, 1)),
            '@' | 'P' => {
                let start = self.cursor_column.min(self.columns - 1);
                let count = parameter(0, 1).min(self.columns - start);
                let blank = self.erase_cell();
                self.erase_images(
                    self.cursor_row as i32,
                    self.cursor_row as i32 + 1,
                    start,
                    self.columns,
                );
                let line = &mut self.cells[self.cursor_row];
                if action == '@' {
                    line.copy_within(start..self.columns - count, start + count);
                    line[start..start + count].fill(blank);
                } else {
                    line.copy_within(start + count..self.columns, start);
                    line[self.columns - count..].fill(blank);
                }
            }
            'n' if values.first() == Some(&6) => self.reply(
                format!(
                    "\x1b[{};{}R",
                    self.cursor_row + 1,
                    self.cursor_column.min(self.columns - 1) + 1
                )
                .as_bytes(),
            ),
            'n' if values.first() == Some(&5) => self.reply(b"\x1b[0n"),
            'c' if values.first().copied().unwrap_or(0) == 0 => self.reply(b"\x1b[?62;4c"),
            't' => match values.first().copied().unwrap_or(0) {
                14 => self.reply(
                    format!(
                        "\x1b[4;{};{}t",
                        self.rows * self.cell_pixels.1,
                        self.columns * self.cell_pixels.0
                    )
                    .as_bytes(),
                ),
                16 => self.reply(
                    format!("\x1b[6;{};{}t", self.cell_pixels.1, self.cell_pixels.0).as_bytes(),
                ),
                18 => self.reply(format!("\x1b[8;{};{}t", self.rows, self.columns).as_bytes()),
                _ => {}
            },
            'm' => {
                let mut sgr = Vec::new();
                for param in params.iter() {
                    if matches!(param.first(), Some(38 | 48)) && param.len() > 1 {
                        // ISO 8613-6: 38:2:[colorspace]:r:g:b, also 38:2:r:g:b.
                        if param.get(1) == Some(&2) && param.len() >= 5 {
                            sgr.extend_from_slice(&param[..2]);
                            sgr.extend_from_slice(&param[param.len() - 3..]);
                        } else if param.get(1) == Some(&5) && param.len() == 3 {
                            sgr.extend_from_slice(param);
                        }
                    } else {
                        sgr.push(param.first().copied().unwrap_or(0));
                    }
                }
                self.set_sgr(&sgr);
            }
            _ => {}
        }
    }

    fn esc_dispatch(&mut self, intermediates: &[u8], _ignore: bool, byte: u8) {
        if intermediates.is_empty() && byte == b'\\' {
            self.finish_sixel();
            return;
        }
        self.pending_sixel = None;
        if !intermediates.is_empty() {
            return;
        }
        match byte {
            b'c' => {
                self.images.clear();
                if let Some(primary) = &mut self.primary_screen {
                    primary.images.clear();
                }
                self.current_link = 0;
                self.sixel_scroll = true;
                self.sixel_right = false;
                self.primary_keyboard = KeyboardMode::default();
                self.alternate_keyboard = KeyboardMode::default();
            }
            b'D' => self.line_feed(),
            b'E' => {
                self.cursor_column = 0;
                self.line_feed();
            }
            b'M' => {
                if self.cursor_row == self.scroll_top {
                    self.scroll_down(1);
                } else {
                    self.cursor_row = self.cursor_row.saturating_sub(1);
                }
            }
            b'7' => self.saved_cursor = (self.cursor_column, self.cursor_row),
            b'8' => {
                self.cursor_column = self.saved_cursor.0.min(self.columns - 1);
                self.cursor_row = self.saved_cursor.1.min(self.rows - 1);
            }
            _ => {}
        }
    }
}

fn ansi_color(index: usize) -> Rgb {
    const PALETTE: [Rgb; 16] = [
        Rgb(24, 29, 43),
        Rgb(255, 94, 119),
        Rgb(38, 220, 167),
        Rgb(255, 196, 87),
        Rgb(92, 156, 255),
        Rgb(205, 115, 255),
        Rgb(52, 220, 235),
        Rgb(204, 218, 238),
        Rgb(82, 94, 119),
        Rgb(255, 123, 145),
        Rgb(82, 240, 190),
        Rgb(255, 215, 125),
        Rgb(124, 180, 255),
        Rgb(220, 150, 255),
        Rgb(96, 238, 250),
        Rgb(246, 249, 255),
    ];
    PALETTE[index.min(15)]
}

fn indexed_color(index: usize) -> Rgb {
    if index < 16 {
        return ansi_color(index);
    }
    if index < 232 {
        let cube = index - 16;
        let component = |value: usize| {
            if value == 0 {
                0
            } else {
                (55 + value * 40) as u8
            }
        };
        return Rgb(
            component(cube / 36),
            component((cube / 6) % 6),
            component(cube % 6),
        );
    }
    let gray = (8 + index.saturating_sub(232).min(23) * 10) as u8;
    Rgb(gray, gray, gray)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sustained_output_and_resizes_keep_history_within_budget() {
        let mut grid = TerminalGrid::new(80, 24);
        for cycle in 0..30 {
            grid.resize(if cycle % 2 == 0 { 512 } else { 80 }, 24);
            for _ in 0..500 {
                grid.feed(b"streamed output\r\n");
            }
            assert!(grid.scrollback_len() <= SCROLLBACK_LIMIT);
            let retained: usize = grid
                .scrollback
                .iter()
                .map(|row| row.capacity() * std::mem::size_of::<Cell>())
                .sum();
            assert_eq!(grid.scrollback_bytes, retained);
            assert!(retained <= SCROLLBACK_BYTES);
        }
        grid.feed(b"\x1b[3J");
        assert_eq!(grid.scrollback_bytes, 0);
        assert!(grid.scrollback.is_empty());
    }

    #[test]
    fn alternate_screen_preserves_shell_and_blocks_history() {
        let mut grid = TerminalGrid::new(12, 3);
        grid.feed(b"history\r\nshell\r\nprompt");
        grid.feed(b"\x1b[?1049h");
        assert!(grid.alternate_screen_active());
        assert_eq!(grid.scrollback_len(), 0);
        assert!(
            grid.visible_line(0, usize::MAX)
                .iter()
                .all(|c| c.character == ' ')
        );
        for _ in 0..100 {
            grid.feed(b"editor\r\n");
        }
        assert_eq!(grid.scrollback_len(), 0);
        grid.resize(14, 4);
        grid.feed(b"\x1b[?1049l");
        assert!(!grid.alternate_screen_active());
        assert_eq!(grid.dimensions(), (14, 4));
        let first: String = grid
            .visible_line(0, 0)
            .iter()
            .map(|c| c.character)
            .collect();
        assert!(first.starts_with("history"));
        assert_eq!(grid.cursor(), (6, 2));
    }

    #[test]
    fn scroll_region_and_reverse_index_leave_outside_lines_intact() {
        let mut grid = TerminalGrid::new(8, 4);
        grid.feed(b"header\r\none\r\ntwo\r\nfooter");
        grid.feed(b"\x1b[2;3r\x1b[3;1H\n");
        assert_eq!(grid.cells[0][0].character, 'h');
        assert_eq!(grid.cells[1][0].character, 't');
        assert_eq!(grid.cells[3][0].character, 'f');
        grid.feed(b"\x1b[2;1H\x1bM");
        assert_eq!(grid.cells[1][0].character, ' ');
        assert_eq!(grid.cells[2][0].character, 't');
    }

    #[test]
    fn oversized_dimensions_are_bounded() {
        let mut grid = TerminalGrid::new(usize::MAX, usize::MAX);
        assert_eq!(grid.dimensions(), (MAX_COLUMNS, MAX_ROWS));
        grid.resize(0, 0);
        assert_eq!(grid.dimensions(), (1, 1));
        grid.resize(usize::MAX, usize::MAX);
        assert_eq!(grid.dimensions(), (MAX_COLUMNS, MAX_ROWS));
    }
}
