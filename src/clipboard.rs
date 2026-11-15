use arboard::{Clipboard, Error, ImageData};
use image::{
    ExtendedColorType, ImageEncoder,
    codecs::png::{CompressionType, FilterType, PngEncoder},
};
use tempfile::NamedTempFile;

const MAX_IMAGES: usize = 32;
const MAX_PIXELS: usize = 16 * 1024 * 1024;
const MAX_SAVED_BYTES: u64 = 64 * 1024 * 1024;

/// Keep image paths readable for the child session; delete files on normal exit.
/// Never evict an image that a running application may still need to read.
#[derive(Default)]
pub struct ClipboardPaste {
    images: Vec<NamedTempFile>,
    saved_bytes: u64,
}

impl ClipboardPaste {
    pub fn read(&mut self) -> Result<String, String> {
        let mut clipboard = Clipboard::new().map_err(|error| error.to_string())?;
        // Some X11 owners report an unsupported image target as an unknown
        // type error rather than ContentNotAvailable. Still try text.
        let image_error = match clipboard.get_image() {
            Ok(image) => return self.save_image(image),
            Err(error) => error,
        };
        clipboard.get_text().map_err(|text_error| {
            if matches!(image_error, Error::ContentNotAvailable)
                && matches!(text_error, Error::ContentNotAvailable)
            {
                "Clipboard contains no text or supported image".to_owned()
            } else {
                format!("Image read failed: {image_error}; text read failed: {text_error}")
            }
        })
    }

    fn save_image(&mut self, image: ImageData<'_>) -> Result<String, String> {
        if self.images.len() >= MAX_IMAGES || self.saved_bytes >= MAX_SAVED_BYTES {
            return Err("Image paste limit reached (32 images / 64 MiB per window)".to_owned());
        }
        let pixels = image
            .width
            .checked_mul(image.height)
            .filter(|pixels| *pixels > 0 && *pixels <= MAX_PIXELS)
            .ok_or("Clipboard image is too large (maximum 16 megapixels)")?;
        if pixels.checked_mul(4) != Some(image.bytes.len()) {
            return Err("Clipboard image has invalid RGBA dimensions".to_owned());
        }
        let mut file = tempfile::Builder::new()
            .prefix("ripple-paste-")
            .suffix(".png")
            .tempfile()
            .map_err(|error| format!("Could not create clipboard image: {error}"))?;
        PngEncoder::new_with_quality(
            file.as_file_mut(),
            CompressionType::Fast,
            FilterType::Adaptive,
        )
        .write_image(
            &image.bytes,
            image.width as u32,
            image.height as u32,
            ExtendedColorType::Rgba8,
        )
        .map_err(|error| format!("Could not save clipboard image: {error}"))?;
        let bytes = file
            .as_file()
            .metadata()
            .map_err(|error| error.to_string())?
            .len();
        if bytes > MAX_SAVED_BYTES - self.saved_bytes {
            return Err("Image paste would exceed the 64 MiB per-window limit".to_owned());
        }
        let path = file
            .path()
            .to_str()
            .ok_or("Clipboard image path is not valid UTF-8")?;
        // A shell must receive a single literal filename. Ordinary /tmp paths
        // stay unquoted so applications can recognize a pasted image path too.
        let pasted = if path
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"/_-.".contains(&byte))
        {
            path.to_owned()
        } else {
            format!("'{}'", path.replace('\'', "'\\''"))
        };
        self.saved_bytes += bytes;
        self.images.push(file);
        Ok(pasted)
    }
}
