use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};

use portable_pty::{CommandBuilder, MasterPty, NativePtySystem, PtySize, PtySystem};
use crate::config::Config;

pub struct PtyReader {
    reader: Box<dyn Read + Send>,
    master: Box<dyn MasterPty + Send>,
    size: Arc<Mutex<PtySize>>,
    resize_needed: Arc<AtomicBool>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
}

pub struct PtyWriter {
    writer: Box<dyn Write + Send>,
    size: Arc<Mutex<PtySize>>,
    resize_needed: Arc<AtomicBool>,
    master_ptr: *const (dyn MasterPty + Send),
}

unsafe impl Send for PtyWriter {}

pub fn spawn(config: &Config, cell_width: f64, cell_height: f64) -> (PtyReader, PtyWriter) {
    let pty_system = NativePtySystem::default();

    let pixel_width = (config.cols as f64 * cell_width) as u16 + 2 * config.padding_x as u16;
    let pixel_height = (config.rows as f64 * cell_height) as u16 + 2 * config.padding_y as u16;

    let size = PtySize {
        rows: config.rows,
        cols: config.cols,
        pixel_width: pixel_width.max(80),
        pixel_height: pixel_height.max(24),
    };

    let pair = pty_system.openpty(size).expect("failed to open PTY");

    let cmd = CommandBuilder::new(&config.shell);
    let child = pair.slave.spawn_command(cmd).expect("failed to spawn shell");

    let reader = pair.master.try_clone_reader().expect("failed to clone reader");
    let writer = pair.master.take_writer().expect("failed to get writer");

    let size_arc = Arc::new(Mutex::new(size));
    let resize_flag = Arc::new(AtomicBool::new(false));

    let master_ptr: *const (dyn MasterPty + Send) = &*pair.master as *const (dyn MasterPty + Send);

    (
        PtyReader {
            reader,
            master: pair.master,
            size: size_arc.clone(),
            resize_needed: resize_flag.clone(),
            child,
        },
        PtyWriter {
            writer,
            size: size_arc,
            resize_needed: resize_flag,
            master_ptr,
        },
    )
}

impl PtyReader {
    pub fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.reader.read(buf)
    }

    pub fn is_alive(&mut self) -> bool {
        self.child.try_wait().map_or(true, |s| s.is_none())
    }

    pub fn apply_resize(&mut self) {
        if self.resize_needed.swap(false, Ordering::SeqCst) {
            let size = *self.size.lock().unwrap();
            if let Err(e) = self.master.resize(size) {
                log::error!("resize failed: {e}");
            }
        }
    }
}

impl PtyWriter {
    pub fn write(&mut self, data: &[u8]) -> std::io::Result<()> {
        self.writer.write_all(data)?;
        self.writer.flush()?;
        Ok(())
    }

    pub fn resize(&mut self, rows: u16, cols: u16) {
        if let Ok(mut size) = self.size.lock() {
            size.rows = rows;
            size.cols = cols;
            self.resize_needed.store(true, Ordering::SeqCst);
        }
    }
}
