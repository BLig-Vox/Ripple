//! Bounded Sixel decoding. No files, subprocesses or external image decoders.
use std::sync::Arc;

pub const INPUT_LIMIT: usize = 4 * 1024 * 1024;
pub const IMAGE_BYTES: usize = 32 * 1024 * 1024;
pub const IMAGE_COUNT: usize = 32;
const SIDE: usize = 2048;
const WORK: usize = 16 * 1024 * 1024;

pub struct Graphic {
    pub id: u64,
    pub row: i32,
    pub column: usize,
    pub width: usize,
    pub height: usize,
    pub rows: usize,
    pub columns: usize,
    pub pixels: Arc<[u8]>,
}
impl Graphic {
    pub fn overlaps(&self, top: i32, bottom: i32, left: usize, right: usize) -> bool {
        self.row < bottom
            && self.row + self.rows as i32 > top
            && self.column < right
            && self.column + self.columns > left
    }
}

pub struct SixelInput {
    pub bytes: Vec<u8>,
    pub transparent: bool,
    pub rejected: bool,
}
impl SixelInput {
    pub fn put(&mut self, byte: u8) {
        if self.rejected {
            return;
        }
        if self.bytes.len() == INPUT_LIMIT {
            self.bytes = Vec::new();
            self.rejected = true;
        } else {
            self.bytes.push(byte);
        }
    }
}

pub struct Bitmap {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
}

pub fn decode(input: SixelInput, background: [u8; 4]) -> Option<Bitmap> {
    if input.rejected {
        return None;
    }
    let (width, height) = walk(&input.bytes, |_, _, _, _, _, _| {})?;
    if width == 0 || height == 0 {
        return None;
    }
    let mut pixels = vec![0; width.checked_mul(height)?.checked_mul(4)?];
    if !input.transparent {
        for pixel in pixels.chunks_exact_mut(4) {
            pixel.copy_from_slice(&background);
        }
    }
    walk(&input.bytes, |x, y, bits, count, color, _| {
        for bit in 0..6 {
            if bits & (1 << bit) == 0 || y + bit >= height {
                continue;
            }
            for column in x..x + count {
                let offset = ((y + bit) * width + column) * 4;
                pixels[offset..offset + 4].copy_from_slice(&color);
            }
        }
    })?;
    Some(Bitmap {
        width,
        height,
        pixels,
    })
}

fn numbers(data: &[u8], index: &mut usize) -> Option<Vec<usize>> {
    let mut result = Vec::with_capacity(5);
    loop {
        if result.len() == 5 {
            return None;
        }
        let mut value = 0usize;
        while let Some(byte @ b'0'..=b'9') = data.get(*index) {
            value = value
                .checked_mul(10)?
                .checked_add(usize::from(*byte - b'0'))?;
            if value > 1_000_000 {
                return None;
            }
            *index += 1;
        }
        result.push(value);
        if data.get(*index) != Some(&b';') {
            break;
        }
        *index += 1;
    }
    Some(result)
}

fn walk(
    data: &[u8],
    mut paint: impl FnMut(usize, usize, u8, usize, [u8; 4], usize),
) -> Option<(usize, usize)> {
    let mut palette = [[0, 0, 0, 255]; 256];
    const DEFAULT: [[u8; 3]; 16] = [
        [0, 0, 0],
        [51, 51, 204],
        [204, 33, 33],
        [51, 204, 51],
        [204, 51, 204],
        [51, 204, 204],
        [204, 204, 51],
        [135, 135, 135],
        [66, 66, 66],
        [84, 84, 255],
        [255, 84, 84],
        [84, 255, 84],
        [255, 84, 255],
        [84, 255, 255],
        [255, 255, 84],
        [255, 255, 255],
    ];
    for (color, rgb) in palette.iter_mut().zip(DEFAULT) {
        *color = [rgb[0], rgb[1], rgb[2], 255];
    }
    let (mut i, mut x, mut y, mut width, mut height, mut selected, mut work) =
        (0, 0, 0, 0, 0, 15, 0usize);
    while i < data.len() {
        let byte = data[i];
        i += 1;
        match byte {
            b'#' => {
                let p = numbers(data, &mut i)?;
                selected = *p.first()?;
                if selected >= 256 {
                    return None;
                }
                if p.len() > 1 {
                    if p.len() != 5 {
                        return None;
                    }
                    palette[selected] = match p[1] {
                        2 if p[2..].iter().all(|v| *v <= 100) => [
                            (p[2] * 255 / 100) as u8,
                            (p[3] * 255 / 100) as u8,
                            (p[4] * 255 / 100) as u8,
                            255,
                        ],
                        1 if p[2] <= 360 && p[3] <= 100 && p[4] <= 100 => hls(p[2], p[3], p[4]),
                        _ => return None,
                    };
                }
            }
            b'"' => {
                let p = numbers(data, &mut i)?;
                if p.len() != 4 || p[2] > SIDE || p[3] > SIDE {
                    return None;
                }
                width = width.max(p[2]);
                height = height.max(p[3]);
            }
            b'$' => x = 0,
            b'-' => {
                x = 0;
                y += 6;
                if y > SIDE + 5 {
                    return None;
                }
            }
            b'!' | b'?'..=b'~' => {
                let (bits, count) = if byte == b'!' {
                    let p = numbers(data, &mut i)?;
                    if p.len() != 1 {
                        return None;
                    }
                    let next = *data.get(i)?;
                    i += 1;
                    if !(b'?'..=b'~').contains(&next) {
                        return None;
                    }
                    (next - b'?', p[0].max(1))
                } else {
                    (byte - b'?', 1)
                };
                if count > SIDE || x + count > SIDE {
                    return None;
                }
                let used = if bits == 0 {
                    1
                } else {
                    8 - bits.leading_zeros() as usize
                };
                if y + used > SIDE {
                    return None;
                }
                work = work.checked_add(count * bits.count_ones() as usize)?;
                if work > WORK {
                    return None;
                }
                paint(x, y, bits, count, palette[selected], work);
                x += count;
                width = width.max(x);
                height = height.max(y + used);
            }
            b'\r' | b'\n' | b'\t' => {}
            _ => return None,
        }
    }
    Some((width, height))
}

fn hls(hue: usize, light: usize, saturation: usize) -> [u8; 4] {
    let h = ((hue + 240) % 360) as f32 / 60.0;
    let l = light as f32 / 100.0;
    let s = saturation as f32 / 100.0;
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let (r, g, b) = match h as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    [
        ((r + m) * 255.0).round() as u8,
        ((g + m) * 255.0).round() as u8,
        ((b + m) * 255.0).round() as u8,
        255,
    ]
}
