use core::sync::atomic::{AtomicUsize, Ordering};
use critical_section;

pub const BUFFER_SIZE: usize = 512;

pub struct CircularBuffer {
    buffer: [u8; BUFFER_SIZE],
    head: AtomicUsize,
    tail: AtomicUsize,
}

impl CircularBuffer {
    pub fn new() -> Self {
        Self {
            buffer: [0u8; BUFFER_SIZE],
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
        }
    }

    pub fn write(&mut self, data: &[u8]) -> Result<usize, BufferError> {
        let mut bytes_written = 0;

        for &byte in data {
            let head = self.head.load(Ordering::Relaxed);
            let tail = self.tail.load(Ordering::Relaxed);
            let next_head = (head + 1) % BUFFER_SIZE;

            if next_head == tail {
                break;
            }

            self.buffer[head] = byte;
            self.head.store(next_head, Ordering::Relaxed);
            bytes_written += 1;
        }

        Ok(bytes_written)
    }

    pub fn read(&mut self, buffer: &mut [u8]) -> Result<usize, BufferError> {
        let mut bytes_read = 0;

        for i in 0..buffer.len() {
            let head = self.head.load(Ordering::Relaxed);
            let tail = self.tail.load(Ordering::Relaxed);

            if head == tail {
                break;
            }

            buffer[i] = self.buffer[tail];
            self.tail.store((tail + 1) % BUFFER_SIZE, Ordering::Relaxed);
            bytes_read += 1;
        }

        Ok(bytes_read)
    }

    pub fn available(&self) -> usize {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Relaxed);

        if head >= tail {
            head - tail
        } else {
            BUFFER_SIZE - tail + head
        }
    }

    pub fn is_full(&self) -> bool {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Relaxed);
        ((head + 1) % BUFFER_SIZE) == tail
    }

    pub fn is_empty(&self) -> bool {
        self.head.load(Ordering::Relaxed) == self.tail.load(Ordering::Relaxed)
    }
}

#[derive(Debug)]
pub enum BufferError {
    BufferFull,
    BufferEmpty,
}

impl core::fmt::Display for BufferError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            BufferError::BufferFull => write!(f, "Buffer full"),
            BufferError::BufferEmpty => write!(f, "Buffer empty"),
        }
    }
}
