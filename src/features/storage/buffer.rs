//! Thread-safe, bounded micro-batch accumulator for real-time market streams.

use std::sync::Mutex;
use std::time::Instant;
use crate::core::types::AggregatorCandle;

pub struct MicroBatchBuffer {
    buffer: Mutex<Vec<AggregatorCandle>>,
    flush_seconds: u64,
    max_rows: usize,
    last_flush: Mutex<Instant>,
}

impl MicroBatchBuffer {
    pub fn new(flush_seconds: u64, max_rows: usize) -> Self {
        Self {
            buffer: Mutex::new(Vec::with_capacity(max_rows)),
            flush_seconds,
            max_rows,
            last_flush: Mutex::new(Instant::now()),
        }
    }

    /// Adds a candle. Returns Some(Vec<AggregatorCandle>) if flush condition is met.
    pub fn add(&self, candle: AggregatorCandle) -> Option<Vec<AggregatorCandle>> {
        let mut buf = self.buffer.lock().unwrap();
        buf.push(candle);

        let now = Instant::now();
        let mut last = self.last_flush.lock().unwrap();
        let elapsed = now.duration_since(*last).as_secs();

        if buf.len() >= self.max_rows || elapsed >= self.flush_seconds {
            let drained = std::mem::replace(&mut *buf, Vec::with_capacity(self.max_rows));
            *last = now;
            Some(drained)
        } else {
            None
        }
    }

    /// Forces a flush of pending records.
    pub fn flush(&self) -> Option<Vec<AggregatorCandle>> {
        let mut buf = self.buffer.lock().unwrap();
        if buf.is_empty() {
            return None;
        }
        let drained = std::mem::replace(&mut *buf, Vec::with_capacity(self.max_rows));
        let mut last = self.last_flush.lock().unwrap();
        *last = Instant::now();
        Some(drained)
    }

    pub fn pending_count(&self) -> usize {
        self.buffer.lock().unwrap().len()
    }
}
