use eframe::egui::{self, Color32, FontId, Key, Stroke, Vec2};

use crate::{
    clipboard::ClipboardPaste,
    terminal::{Rgb, TerminalGpuRenderer, TerminalGrid, TerminalSession, gpu_cell_paint_callback},
    window_geometry,
};

const INITIAL_COLUMNS: usize = 100;
const INITIAL_ROWS: usize = 30;
const CELL_WIDTH: f32 = 9.2;
const CELL_HEIGHT: f32 = 19.0;
const TERMINAL_FONT_SIZE: f32 = 15.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CellPosition {
    row: usize,
    column: usize,
}

pub struct RippleApp {
    terminal: TerminalGrid,
    session: Option<TerminalSession>,
    session_error: Option<String>,
    gpu_error: Option<String>,
    error: Option<String>,
    selection: Option<(CellPosition, CellPosition)>,
    scroll_offset: usize,
    wheel_remainder: f32,
    clipboard_paste: ClipboardPaste,
}

impl RippleApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        window_geometry::apply_initial_geometry(cc);
        let terminal = TerminalGrid::new(INITIAL_COLUMNS, INITIAL_ROWS);
        let repaint_context = cc.egui_ctx.clone();
        let (session, session_error) =
            match TerminalSession::spawn(INITIAL_COLUMNS, INITIAL_ROWS, move || {
                repaint_context.request_repaint()
            }) {
                Ok(session) => (Some(session), None),
                Err(error) => (None, Some(error)),
            };
        let gpu_error = TerminalGpuRenderer::install(cc).err();
        Self {
            terminal,
            session,
            session_error,
            gpu_error,
            error: None,
            selection: None,
            scroll_offset: 0,
            wheel_remainder: 0.0,
            clipboard_paste: ClipboardPaste::default(),
        }
    }

    fn poll_output(&mut self, ctx: &egui::Context) {
        let Some(session) = &mut self.session else {
            return;
        };
        let chunks = session.drain_output(24);
        if !chunks.is_empty() {
            for chunk in chunks {
                let was_alternate = self.terminal.alternate_screen_active();
                self.terminal.feed(&chunk);
                if was_alternate != self.terminal.alternate_screen_active()
                    || self.terminal.alternate_screen_active()
                {
                    self.scroll_offset = 0;
                    self.selection = None;
                }
                let replies = self.terminal.take_replies();
                if !replies.is_empty() {
                    if let Err(error) = session.write_input(&replies) {
                        self.error = Some(format!("PTY reply error: {error}"));
                    }
                }
            }
            ctx.request_repaint();
        }
        if !session.child_running() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            self.session = None;
        }
    }

    fn send_input(&mut self, bytes: &[u8]) {
        self.scroll_offset = 0;
        if let Some(session) = &mut self.session {
            if let Err(error) = session.write_input(bytes) {
                self.error = Some(format!("PTY input error: {error}"));
            }
        }
    }

    fn handle_keyboard(&mut self, ctx: &egui::Context) {
        let events = ctx.input(|input| input.events.clone());
        for event in events {
            match event {
                egui::Event::Text(text) => {
                    if !text.is_empty() {
                        self.send_input(text.as_bytes());
                    }
                }
                egui::Event::Paste(text) => self.paste_text(&text),
                egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => {
                    if modifiers.ctrl || modifiers.command {
                        if modifiers.shift && key == Key::C {
                            self.copy_selection(ctx);
                        } else if modifiers.shift && key == Key::V {
                            self.paste_clipboard();
                        } else if let Some(byte) = control_byte(key) {
                            self.send_input(&[byte]);
                        }
                    } else if let Some(sequence) = key_sequence(key) {
                        if self.terminal.application_cursor
                            && matches!(
                                key,
                                Key::ArrowUp
                                    | Key::ArrowDown
                                    | Key::ArrowLeft
                                    | Key::ArrowRight
                                    | Key::Home
                                    | Key::End
                            )
                        {
                            self.send_input(&[0x1b, b'O', sequence[sequence.len() - 1]]);
                        } else {
                            self.send_input(sequence);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn copy_selection(&mut self, ctx: &egui::Context) {
        if let Some(text) = self.selected_text() {
            ctx.copy_text(text);
        }
    }

    fn paste_clipboard(&mut self) {
        match self.clipboard_paste.read() {
            Ok(text) => self.paste_text(&text),
            Err(error) => self.error = Some(format!("Clipboard unavailable: {error}")),
        }
    }

    fn paste_text(&mut self, text: &str) {
        self.error = None;
        if self.terminal.bracketed_paste {
            let paste = format!("\x1b[200~{}\x1b[201~", text.replace("\x1b", ""));
            self.send_input(paste.as_bytes());
        } else {
            self.send_input(text.as_bytes());
        }
    }

    fn selected_text(&self) -> Option<String> {
        let (first, last) = self.selection?;
        let (start, end) = ordered_positions(first, last);
        let (_, rows) = self.terminal.dimensions();
        let mut result = String::new();
        for row_index in start.row..=end.row.min(rows - 1) {
            let line = self.terminal.visible_line(row_index, self.scroll_offset);
            let first_column = if row_index == start.row {
                start.column
            } else {
                0
            };
            let last_column = if row_index == end.row {
                end.column.saturating_add(1)
            } else {
                line.len()
            };
            let text: String = line
                .iter()
                .take(last_column)
                .skip(first_column)
                .map(|cell| cell.character)
                .collect();
            result.push_str(text.trim_end());
            if row_index < end.row {
                result.push('\n');
            }
        }
        Some(result)
    }

    fn update_terminal_size(&mut self, ctx: &egui::Context, available: Vec2) {
        let columns = ((available.x - 16.0) / CELL_WIDTH).floor().max(1.0) as usize;
        let rows = ((available.y - 16.0) / CELL_HEIGHT).floor().max(1.0) as usize;
        if self.terminal.dimensions() == (columns, rows) {
            return;
        }
        self.terminal.resize(columns, rows);
        let (columns, rows) = self.terminal.dimensions();
        self.selection = None;
        if let Some(session) = &mut self.session {
            if let Err(error) = session.resize(columns, rows) {
                self.error = Some(error);
            }
        }
        ctx.request_repaint();
    }

    fn draw_error(&self, ui: &egui::Ui, rect: egui::Rect) {
        if let Some(error) = self
            .error
            .as_ref()
            .or(self.session_error.as_ref())
            .or(self.gpu_error.as_ref())
        {
            ui.painter_at(rect).text(
                rect.left_bottom() + Vec2::new(8.0, -8.0),
                egui::Align2::LEFT_BOTTOM,
                error,
                FontId::monospace(13.0),
                Color32::from_rgb(230, 150, 150),
            );
        }
    }

    fn draw_terminal(&self, ui: &mut egui::Ui, rect: egui::Rect) {
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, Color32::from_rgb(24, 24, 24));
        let origin = Vec2::splat(8.0);
        if self.gpu_error.is_none() {
            painter.add(gpu_cell_paint_callback(
                rect,
                &self.terminal,
                CELL_WIDTH,
                CELL_HEIGHT,
                origin,
                self.scroll_offset,
                self.selection
                    .map(|(start, end)| ((start.row, start.column), (end.row, end.column))),
            ));
        }
        let font = FontId::monospace(TERMINAL_FONT_SIZE);

        for row_index in 0..self.terminal.dimensions().1 {
            let row = self.terminal.visible_line(row_index, self.scroll_offset);
            let mut column_index = 0;
            while column_index < row.len().min(self.terminal.dimensions().0) {
                let run_start = column_index;
                let foreground = row[column_index].foreground;
                column_index += 1;
                while column_index < row.len().min(self.terminal.dimensions().0)
                    && row[column_index].foreground == foreground
                {
                    column_index += 1;
                }
                let run = &row[run_start..column_index];
                if run.iter().any(|cell| cell.character != ' ') {
                    let text: String = run.iter().map(|cell| cell.character).collect();
                    let position = rect.min
                        + origin
                        + Vec2::new(
                            run_start as f32 * CELL_WIDTH,
                            row_index as f32 * CELL_HEIGHT,
                        );
                    painter.text(
                        position,
                        egui::Align2::LEFT_TOP,
                        text,
                        font.clone(),
                        to_color32(foreground),
                    );
                }
            }
        }

        if self.gpu_error.is_some() && self.scroll_offset == 0 && self.terminal.cursor_visible {
            let (cursor_column, cursor_row) = self.terminal.cursor();
            let cursor_position = rect.min
                + origin
                + Vec2::new(
                    cursor_column as f32 * CELL_WIDTH,
                    cursor_row as f32 * CELL_HEIGHT,
                );
            painter.line_segment(
                [
                    cursor_position,
                    cursor_position + Vec2::new(0.0, CELL_HEIGHT - 2.0),
                ],
                Stroke::new(2.0, Color32::from_rgb(210, 210, 210)),
            );
        }
    }
}

impl eframe::App for RippleApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll_output(&ctx);
        if self.session.is_some() {
            self.handle_keyboard(&ctx);
        }

        let terminal = ui.max_rect();

        self.update_terminal_size(&ctx, terminal.size());
        let response = ui.allocate_rect(terminal, egui::Sense::click_and_drag());
        if response.clicked() || !response.has_focus() {
            response.request_focus();
        }
        if response.drag_started_by(egui::PointerButton::Primary) {
            if let Some(pointer) = response.interact_pointer_pos() {
                let point = pointer_to_cell(pointer, terminal, &self.terminal);
                self.selection = Some((point, point));
            }
        } else if response.dragged_by(egui::PointerButton::Primary) {
            if let (Some((start, _)), Some(pointer)) =
                (self.selection, response.interact_pointer_pos())
            {
                let end = pointer_to_cell(pointer, terminal, &self.terminal);
                self.selection = Some((start, end));
            }
        } else if response.clicked() {
            self.selection = None;
        }
        if response.hovered() {
            self.wheel_remainder += ctx.input(|input| input.smooth_scroll_delta.y);
            let steps = (self.wheel_remainder / CELL_HEIGHT).trunc() as isize;
            if steps != 0 {
                self.wheel_remainder -= steps as f32 * CELL_HEIGHT;
                if self.terminal.mouse_tracking {
                    let pointer = ctx
                        .input(|input| input.pointer.hover_pos())
                        .unwrap_or(terminal.min);
                    let cell = pointer_to_cell(pointer, terminal, &self.terminal);
                    for _ in 0..steps.unsigned_abs().min(20) {
                        let button = if steps > 0 { 64 } else { 65 };
                        if self.terminal.sgr_mouse {
                            self.send_input(
                                format!("\x1b[<{};{};{}M", button, cell.column + 1, cell.row + 1)
                                    .as_bytes(),
                            );
                        } else {
                            self.send_input(&[
                                0x1b,
                                b'[',
                                b'M',
                                button + 32,
                                (cell.column + 33).min(255) as u8,
                                (cell.row + 33).min(255) as u8,
                            ]);
                        }
                    }
                } else if self.terminal.alternate_screen_active() {
                    // Applications without mouse reporting receive cursor keys,
                    // never shell scrollback from the primary screen.
                    for _ in 0..steps.unsigned_abs().min(20) {
                        let sequence = match (steps > 0, self.terminal.application_cursor) {
                            (true, true) => b"\x1bOA",
                            (false, true) => b"\x1bOB",
                            (true, false) => b"\x1b[A",
                            (false, false) => b"\x1b[B",
                        };
                        self.send_input(sequence);
                    }
                } else {
                    self.scroll_offset = (self.scroll_offset as isize + steps)
                        .clamp(0, self.terminal.scrollback_len() as isize)
                        as usize;
                }
            }
        }
        self.draw_terminal(ui, terminal);
        self.draw_error(ui, terminal);
    }
}

fn to_color32(color: Rgb) -> Color32 {
    Color32::from_rgb(color.0, color.1, color.2)
}

fn ordered_positions(first: CellPosition, second: CellPosition) -> (CellPosition, CellPosition) {
    if (first.row, first.column) <= (second.row, second.column) {
        (first, second)
    } else {
        (second, first)
    }
}

fn pointer_to_cell(pointer: egui::Pos2, rect: egui::Rect, terminal: &TerminalGrid) -> CellPosition {
    let (columns, rows) = terminal.dimensions();
    let column = ((pointer.x - rect.left() - 8.0) / CELL_WIDTH).floor() as isize;
    let row = ((pointer.y - rect.top() - 8.0) / CELL_HEIGHT).floor() as isize;
    CellPosition {
        row: row.clamp(0, rows.saturating_sub(1) as isize) as usize,
        column: column.clamp(0, columns.saturating_sub(1) as isize) as usize,
    }
}

fn control_byte(key: Key) -> Option<u8> {
    match key {
        Key::A => Some(1),
        Key::B => Some(2),
        Key::C => Some(3),
        Key::D => Some(4),
        Key::E => Some(5),
        Key::F => Some(6),
        Key::G => Some(7),
        Key::H => Some(8),
        Key::I => Some(9),
        Key::J => Some(10),
        Key::K => Some(11),
        Key::L => Some(12),
        Key::M => Some(13),
        Key::N => Some(14),
        Key::O => Some(15),
        Key::P => Some(16),
        Key::Q => Some(17),
        Key::R => Some(18),
        Key::S => Some(19),
        Key::T => Some(20),
        Key::U => Some(21),
        Key::V => Some(22),
        Key::W => Some(23),
        Key::X => Some(24),
        Key::Y => Some(25),
        Key::Z => Some(26),
        Key::Space => Some(0),
        _ => None,
    }
}

fn key_sequence(key: Key) -> Option<&'static [u8]> {
    match key {
        Key::Enter => Some(b"\r"),
        Key::Backspace => Some(b"\x7f"),
        Key::Tab => Some(b"\t"),
        Key::Escape => Some(b"\x1b"),
        Key::ArrowUp => Some(b"\x1b[A"),
        Key::ArrowDown => Some(b"\x1b[B"),
        Key::ArrowRight => Some(b"\x1b[C"),
        Key::ArrowLeft => Some(b"\x1b[D"),
        Key::Home => Some(b"\x1b[H"),
        Key::End => Some(b"\x1b[F"),
        Key::Delete => Some(b"\x1b[3~"),
        Key::PageUp => Some(b"\x1b[5~"),
        Key::PageDown => Some(b"\x1b[6~"),
        _ => None,
    }
}
