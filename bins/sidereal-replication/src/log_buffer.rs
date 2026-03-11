use std::collections::VecDeque;
use std::fs::File;
use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use bevy::prelude::Resource;

const DEFAULT_LOG_BUFFER_CAPACITY: usize = 10_000;
static LOG_TO_STDERR: AtomicBool = AtomicBool::new(true);

#[derive(Debug, Clone, Default, Resource)]
pub struct SharedLogBuffer {
    inner: Arc<Mutex<BoundedLogBuffer>>,
}

impl SharedLogBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(BoundedLogBuffer::new(capacity))),
        }
    }

    pub fn snapshot(&self) -> Vec<String> {
        let inner = self.inner.lock().expect("log buffer lock poisoned");
        inner.lines.iter().cloned().collect()
    }
}

#[derive(Debug, Default)]
struct BoundedLogBuffer {
    capacity: usize,
    lines: VecDeque<String>,
    partial_line: String,
}

impl BoundedLogBuffer {
    fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            lines: VecDeque::with_capacity(capacity.max(1)),
            partial_line: String::new(),
        }
    }

    fn push(&mut self, line: String) {
        if self.lines.len() == self.capacity {
            self.lines.pop_front();
        }
        self.lines.push_back(line);
    }

    fn push_bytes(&mut self, bytes: &[u8]) {
        let rendered = String::from_utf8_lossy(bytes);
        for ch in rendered.chars() {
            match ch {
                '\n' => {
                    let line = std::mem::take(&mut self.partial_line);
                    self.push(line);
                }
                '\r' => {}
                other => self.partial_line.push(other),
            }
        }
    }
}

#[derive(Clone)]
pub struct ReplicationLogFanout {
    file: Arc<Mutex<File>>,
    buffer: SharedLogBuffer,
}

impl ReplicationLogFanout {
    pub fn new(file: File, buffer: SharedLogBuffer) -> Self {
        Self {
            file: Arc::new(Mutex::new(file)),
            buffer,
        }
    }

    pub fn make_writer(&self) -> ReplicationLogWriter {
        ReplicationLogWriter {
            file: Arc::clone(&self.file),
            buffer: self.buffer.clone(),
        }
    }
}

pub struct ReplicationLogWriter {
    file: Arc<Mutex<File>>,
    buffer: SharedLogBuffer,
}

impl Write for ReplicationLogWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if LOG_TO_STDERR.load(Ordering::Relaxed) {
            let mut stderr = io::stderr().lock();
            stderr.write_all(buf)?;
        }
        {
            let mut file = self.file.lock().expect("log file lock poisoned");
            file.write_all(buf)?;
        }
        let mut inner = self.buffer.inner.lock().expect("log buffer lock poisoned");
        inner.push_bytes(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if LOG_TO_STDERR.load(Ordering::Relaxed) {
            io::stderr().lock().flush()?;
        }
        let mut file = self.file.lock().expect("log file lock poisoned");
        file.flush()
    }
}

static REPLICATION_LOG_BUFFER: OnceLock<SharedLogBuffer> = OnceLock::new();

pub fn init_global_log_buffer() -> SharedLogBuffer {
    REPLICATION_LOG_BUFFER
        .get_or_init(|| SharedLogBuffer::new(DEFAULT_LOG_BUFFER_CAPACITY))
        .clone()
}

pub fn set_log_to_stderr(enabled: bool) {
    LOG_TO_STDERR.store(enabled, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::SharedLogBuffer;

    #[test]
    fn shared_log_buffer_drops_oldest_when_full() {
        let buffer = SharedLogBuffer::new(2);
        {
            let mut inner = buffer.inner.lock().unwrap();
            inner.push("first".to_string());
            inner.push("second".to_string());
            inner.push("third".to_string());
        }

        assert_eq!(buffer.snapshot(), vec!["second", "third"]);
    }
}
