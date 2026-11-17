//! Keyboard bytes sent to the PTY. Keep text/IME events separate from keys.
use eframe::egui::{Key, Modifiers};

pub const DISAMBIGUATE: u8 = 1;
const STACK_LIMIT: usize = 16;

#[derive(Default)]
pub struct KeyboardMode {
    flags: u8,
    stack: std::collections::VecDeque<u8>,
}
impl KeyboardMode {
    pub fn flags(&self) -> u8 {
        self.flags
    }
    pub fn set(&mut self, flags: u16, mode: u16) {
        let flags = (flags as u8) & DISAMBIGUATE;
        match mode {
            0 | 1 => self.flags = flags,
            2 => self.flags |= flags,
            3 => self.flags &= !flags,
            _ => {}
        }
    }
    pub fn push(&mut self, flags: u16) {
        if self.stack.len() == STACK_LIMIT {
            self.stack.pop_front();
        }
        self.stack.push_back(self.flags);
        self.set(flags, 1);
    }
    pub fn pop(&mut self, count: u16) {
        let count = usize::from(count.max(1));
        if count > self.stack.len() {
            self.stack.clear();
            self.flags = 0;
        } else {
            for _ in 0..count {
                self.flags = self.stack.pop_back().unwrap_or(0);
            }
        }
    }
}

pub fn encode_key(
    key: Key,
    modifiers: Modifiers,
    flags: u8,
    application_cursor: bool,
) -> Option<Vec<u8>> {
    let ctrl = modifiers.ctrl || modifiers.command;
    let modifier = 1 + u8::from(modifiers.shift) + 2 * u8::from(modifiers.alt) + 4 * u8::from(ctrl);
    // Modified Enter must remain distinct even for apps using legacy input.
    if key == Key::Enter && modifier > 1 {
        return Some(format!("\x1b[13;{modifier}u").into_bytes());
    }
    let enhanced = flags & DISAMBIGUATE != 0;
    if enhanced
        && (ctrl
            || modifiers.alt
            || (modifiers.shift && matches!(key, Key::Tab | Key::Backspace))
            || key == Key::Escape)
    {
        if let Some(code) = unicode_key(key) {
            return Some(format!("\x1b[{code};{modifier}u").into_bytes());
        }
    }
    let cursor = matches!(
        key,
        Key::ArrowUp | Key::ArrowDown | Key::ArrowLeft | Key::ArrowRight | Key::Home | Key::End
    );
    let function = matches!(key, Key::F1 | Key::F2 | Key::F4);
    if (cursor || function) && (modifier > 1 || enhanced) {
        let sequence = key_sequence(key)?;
        return Some(
            format!("\x1b[1;{modifier}{}", sequence[sequence.len() - 1] as char).into_bytes(),
        );
    }
    if matches!(
        key,
        Key::Insert
            | Key::Delete
            | Key::PageUp
            | Key::PageDown
            | Key::F3
            | Key::F5
            | Key::F6
            | Key::F7
            | Key::F8
            | Key::F9
            | Key::F10
            | Key::F11
            | Key::F12
    ) && (modifier > 1 || enhanced)
    {
        let sequence = if key == Key::F3 {
            b"\x1b[13~".as_slice()
        } else {
            key_sequence(key)?
        };
        let number = std::str::from_utf8(&sequence[2..sequence.len() - 1]).ok()?;
        return Some(format!("\x1b[{number};{modifier}~").into_bytes());
    }
    if ctrl {
        return control_byte(key).map(|byte| vec![byte]);
    }
    if key == Key::Tab && modifiers.shift {
        return Some(b"\x1b[Z".to_vec());
    }
    let sequence = key_sequence(key)?;
    let mut result = if application_cursor && cursor {
        vec![0x1b, b'O', sequence[sequence.len() - 1]]
    } else {
        sequence.to_vec()
    };
    if modifiers.alt && !cursor {
        result.insert(0, 0x1b);
    }
    Some(result)
}

// egui emits a Text event after an Alt+printable Key event. Suppress that one
// duplicate when the key was already encoded through the enhanced protocol.
pub fn encodes_alt_text(key: Key, modifiers: Modifiers, flags: u8) -> bool {
    flags & DISAMBIGUATE != 0
        && modifiers.alt
        && !modifiers.ctrl
        && !modifiers.command
        && unicode_key(key).is_some_and(|code| code >= 32 && code != 127)
}

fn unicode_key(key: Key) -> Option<u32> {
    match key {
        Key::Enter => Some(13),
        Key::Tab => Some(9),
        Key::Backspace => Some(127),
        Key::Escape => Some(27),
        Key::Space => Some(32),
        Key::Colon => Some(59),
        Key::Comma => Some(44),
        Key::Backslash => Some(92),
        Key::Slash => Some(47),
        Key::Pipe => Some(92),
        Key::Questionmark => Some(47),
        Key::Exclamationmark => Some(49),
        Key::OpenBracket => Some(91),
        Key::CloseBracket => Some(93),
        Key::OpenCurlyBracket => Some(91),
        Key::CloseCurlyBracket => Some(93),
        Key::Backtick => Some(96),
        Key::Minus => Some(45),
        Key::Period => Some(46),
        Key::Plus => Some(61),
        Key::Equals => Some(61),
        Key::Semicolon => Some(59),
        Key::Quote => Some(39),
        _ => {
            let name = key.name();
            if name.len() == 1 {
                Some(u32::from(name.as_bytes()[0].to_ascii_lowercase()))
            } else {
                None
            }
        }
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
        Key::Insert => Some(b"\x1b[2~"),
        Key::Delete => Some(b"\x1b[3~"),
        Key::F1 => Some(b"\x1bOP"),
        Key::F2 => Some(b"\x1bOQ"),
        Key::F3 => Some(b"\x1bOR"),
        Key::F4 => Some(b"\x1bOS"),
        Key::F5 => Some(b"\x1b[15~"),
        Key::F6 => Some(b"\x1b[17~"),
        Key::F7 => Some(b"\x1b[18~"),
        Key::F8 => Some(b"\x1b[19~"),
        Key::F9 => Some(b"\x1b[20~"),
        Key::F10 => Some(b"\x1b[21~"),
        Key::F11 => Some(b"\x1b[23~"),
        Key::F12 => Some(b"\x1b[24~"),
        Key::PageUp => Some(b"\x1b[5~"),
        Key::PageDown => Some(b"\x1b[6~"),
        _ => None,
    }
}
