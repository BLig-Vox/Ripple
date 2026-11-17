use std::{
    io::{Read, Write},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use crossbeam_channel::{Receiver, SendTimeoutError, bounded};
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};

const OUTPUT_BUFFER_CHUNKS: usize = 32;
const READER_CHUNK_SIZE: usize = 8 * 1024;

pub struct TerminalSession {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send>,
    output: Receiver<Vec<u8>>,
    columns: u16,
    rows: u16,
    cell_pixels: (usize, usize),
    stop: Arc<AtomicBool>,
    reader_thread: Option<thread::JoinHandle<()>>,
}

impl TerminalSession {
    pub fn spawn<F>(columns: usize, rows: usize, repaint: F) -> Result<Self, String>
    where
        F: Fn() + Send + Sync + 'static,
    {
        let columns = columns.clamp(1, u16::MAX as usize) as u16;
        let rows = rows.clamp(1, u16::MAX as usize) as u16;
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols: columns,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|error| format!("Could not open a PTY: {error}"))?;

        let mut command = CommandBuilder::new_default_prog();
        if let Some(home) = std::env::var_os("HOME") {
            command.cwd(PathBuf::from(home));
        }
        command.env("TERM", "xterm-256color");
        command.env("COLORTERM", "truecolor");
        command.env("TERM_PROGRAM", "ripple");
        command.env("TERM_PROGRAM_VERSION", env!("CARGO_PKG_VERSION"));
        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|error| format!("Could not read from the PTY: {error}"))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|error| format!("Could not write to the PTY: {error}"))?;
        let mut child = pair
            .slave
            .spawn_command(command)
            .map_err(|error| format!("Could not start the default shell: {error}"))?;
        drop(pair.slave);
        let reader_fd = pair.master.as_raw_fd().ok_or_else(|| {
            let _ = child.kill();
            let _ = child.wait();
            "The Linux PTY has no readable descriptor".to_owned()
        })?;
        let stop = Arc::new(AtomicBool::new(false));
        let reader_stop = Arc::clone(&stop);
        let (sender, output) = bounded(OUTPUT_BUFFER_CHUNKS);
        let repaint = Arc::new(repaint);
        let reader_repaint = Arc::clone(&repaint);

        let reader_thread = thread::Builder::new()
            .name("ripple-pty-reader".to_owned())
            .spawn(move || {
                let mut buffer = vec![0; READER_CHUNK_SIZE];
                'read: while !reader_stop.load(Ordering::Acquire) {
                    let mut descriptor = libc::pollfd {
                        fd: reader_fd,
                        events: libc::POLLIN,
                        revents: 0,
                    };
                    // SAFETY: the master descriptor stays owned by the session until
                    // Drop joins this thread. poll only borrows it for this call.
                    let ready = unsafe { libc::poll(&mut descriptor, 1, 50) };
                    if ready < 0 {
                        if std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted
                        {
                            continue;
                        }
                        break;
                    }
                    if ready == 0 || reader_stop.load(Ordering::Acquire) {
                        continue;
                    }
                    match reader.read(&mut buffer) {
                        Ok(0) | Err(_) => break,
                        Ok(count) => {
                            let mut chunk = buffer[..count].to_vec();
                            loop {
                                if reader_stop.load(Ordering::Acquire) {
                                    break 'read;
                                }
                                match sender.send_timeout(chunk, Duration::from_millis(50)) {
                                    Ok(()) => break,
                                    Err(SendTimeoutError::Timeout(returned)) => chunk = returned,
                                    Err(SendTimeoutError::Disconnected(_)) => break 'read,
                                }
                            }
                            reader_repaint();
                        }
                    }
                }
                // Wake the UI on EOF too, so a quiet shell exit gets observed.
                reader_repaint();
            })
            .map_err(|error| {
                let _ = child.kill();
                let _ = child.wait();
                format!("Could not start the PTY reader: {error}")
            })?;

        Ok(Self {
            master: pair.master,
            writer,
            child,
            output,
            stop,
            reader_thread: Some(reader_thread),
            columns,
            rows,
            cell_pixels: (0, 0),
        })
    }

    pub fn write_input(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        self.writer.write_all(bytes)
    }

    pub fn resize(
        &mut self,
        columns: usize,
        rows: usize,
        cell_width: usize,
        cell_height: usize,
    ) -> Result<(), String> {
        let columns = columns.clamp(1, u16::MAX as usize) as u16;
        let rows = rows.clamp(1, u16::MAX as usize) as u16;
        if columns == self.columns
            && rows == self.rows
            && self.cell_pixels == (cell_width, cell_height)
        {
            return Ok(());
        }
        self.master
            .resize(PtySize {
                rows,
                cols: columns,
                pixel_width: (columns as usize * cell_width).min(u16::MAX as usize) as u16,
                pixel_height: (rows as usize * cell_height).min(u16::MAX as usize) as u16,
            })
            .map_err(|error| format!("Could not resize the PTY: {error}"))?;
        self.columns = columns;
        self.rows = rows;
        self.cell_pixels = (cell_width, cell_height);
        Ok(())
    }

    pub fn drain_output(&self, max_chunks: usize) -> Vec<Vec<u8>> {
        let mut chunks = Vec::with_capacity(max_chunks.min(OUTPUT_BUFFER_CHUNKS));
        while chunks.len() < max_chunks {
            match self.output.try_recv() {
                Ok(chunk) => chunks.push(chunk),
                Err(_) => break,
            }
        }
        chunks
    }

    pub fn child_running(&mut self) -> bool {
        self.child.try_wait().ok().flatten().is_none()
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let _ = self.child.kill();
        if let Some(reader_thread) = self.reader_thread.take() {
            let _ = reader_thread.join();
        }
        // Reap the shell instead of leaving a zombie until Ripple exits.
        let _ = self.child.wait();
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn repeated_sessions_release_child_and_reader_even_with_full_queue() {
        let baseline_threads = std::fs::read_dir("/proc/self/task").unwrap().count();
        let baseline_fds = std::fs::read_dir("/proc/self/fd").unwrap().count();
        for iteration in 0..12 {
            let mut session = TerminalSession::spawn(80, 24, || {}).unwrap();
            let pid = session.child.process_id().unwrap();
            if iteration % 2 == 0 {
                session
                    .write_input(
                        b"while :; do printf '0123456789abcdefghijklmnopqrstuvwxyz\\n'; done\r",
                    )
                    .unwrap();
                let deadline = Instant::now() + Duration::from_secs(5);
                while session.output.len() < OUTPUT_BUFFER_CHUNKS && Instant::now() < deadline {
                    thread::sleep(Duration::from_millis(10));
                }
                assert_eq!(session.output.len(), OUTPUT_BUFFER_CHUNKS);
            }
            let start = Instant::now();
            drop(session);
            assert!(start.elapsed() < Duration::from_secs(2), "shutdown blocked");
            assert!(
                !PathBuf::from(format!("/proc/{pid}")).exists(),
                "child {pid} was not reaped"
            );
            assert_eq!(
                std::fs::read_dir("/proc/self/task").unwrap().count(),
                baseline_threads,
                "reader thread leaked"
            );
            assert_eq!(
                std::fs::read_dir("/proc/self/fd").unwrap().count(),
                baseline_fds,
                "PTY descriptor leaked"
            );
        }
    }
}
