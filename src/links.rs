//! Bounded desktop URL opener. Terminal output never becomes shell code.
use crossbeam_channel::{Sender, bounded};
use std::process::{Command, Stdio};

pub fn safe_url(url: &str) -> bool {
    if url.len() > 2048 || url.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return false;
    }
    let rest = if url
        .get(..8)
        .is_some_and(|s| s.eq_ignore_ascii_case("https://"))
    {
        &url[8..]
    } else if url
        .get(..7)
        .is_some_and(|s| s.eq_ignore_ascii_case("http://"))
    {
        &url[7..]
    } else {
        return false;
    };
    !rest.split(['/', '?', '#']).next().unwrap_or("").is_empty()
}

pub struct LinkOpener {
    sender: Option<Sender<String>>,
}
impl LinkOpener {
    pub fn new() -> Self {
        let (sender, receiver) = bounded::<String>(8);
        let worker = std::thread::Builder::new()
            .name("ripple-links".into())
            .spawn(move || {
                for url in receiver {
                    let result = Command::new("gio")
                        .args(["open", &url])
                        .stdin(Stdio::null())
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .status();
                    if result.is_ok_and(|status| status.success()) {
                        continue;
                    }
                    let _ = Command::new("xdg-open")
                        .arg(&url)
                        .stdin(Stdio::null())
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .status();
                }
            });
        Self {
            sender: worker.ok().map(|_| sender),
        }
    }
    pub fn open(&self, url: String) -> Result<(), &'static str> {
        if !safe_url(&url) {
            return Err("Only HTTP/HTTPS links can be opened");
        }
        self.sender
            .as_ref()
            .ok_or("Could not start desktop link worker")?
            .try_send(url)
            .map_err(|_| "Desktop link queue is busy")
    }
}
