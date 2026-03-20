//! Buffered file writer for session logs.

use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::PathBuf;

use crate::errors::AppError;

/// Buffer size for log writes (in lines).
const BUFFER_SIZE: usize = 100;

/// Buffered writer that periodically flushes to disk.
pub struct BufferedWriter {
    writer: BufWriter<File>,
    buffer_count: usize,
    immediate_flush: bool,
}

impl BufferedWriter {
    /// Create a new buffered writer.
    ///
    /// If `immediate_flush` is true, every line is flushed immediately (verbose mode).
    /// Otherwise, lines are buffered and flushed every BUFFER_SIZE lines.
    pub fn new(path: PathBuf, immediate_flush: bool) -> Result<Self, AppError> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;

        Ok(Self {
            writer: BufWriter::new(file),
            buffer_count: 0,
            immediate_flush,
        })
    }

    /// Write a line to the log.
    pub fn write_line(&mut self, line: &str) -> Result<(), AppError> {
        writeln!(self.writer, "{}", line)?;

        self.buffer_count += 1;

        // Flush if immediate mode or buffer is full
        if self.immediate_flush || self.buffer_count >= BUFFER_SIZE {
            self.flush()?;
            self.buffer_count = 0;
        }

        Ok(())
    }

    /// Flush the buffer to disk.
    pub fn flush(&mut self) -> Result<(), AppError> {
        self.writer.flush()?;
        Ok(())
    }

    /// Update the immediate flush mode.
    pub fn set_immediate_flush(&mut self, immediate: bool) {
        self.immediate_flush = immediate;
    }
}

impl Drop for BufferedWriter {
    fn drop(&mut self) {
        // Ensure we flush on drop
        let _ = self.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn buffered_writer_creates_file() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("test.log");

        let _writer = BufferedWriter::new(path.clone(), false).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn buffered_writer_writes_lines() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("test.log");

        let mut writer = BufferedWriter::new(path.clone(), true).unwrap();
        writer.write_line("Line 1").unwrap();
        writer.write_line("Line 2").unwrap();
        writer.write_line("Line 3").unwrap();

        // Drop to flush
        drop(writer);

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("Line 1"));
        assert!(content.contains("Line 2"));
        assert!(content.contains("Line 3"));
    }

    #[test]
    fn buffered_writer_immediate_flush() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("test.log");

        let mut writer = BufferedWriter::new(path.clone(), true).unwrap();
        writer.write_line("Test line").unwrap();

        // With immediate flush, content should be on disk without explicit flush
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("Test line"));
    }

    #[test]
    fn buffered_writer_appends() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("test.log");

        // Write first batch
        {
            let mut writer = BufferedWriter::new(path.clone(), true).unwrap();
            writer.write_line("First batch").unwrap();
        }

        // Write second batch
        {
            let mut writer = BufferedWriter::new(path.clone(), true).unwrap();
            writer.write_line("Second batch").unwrap();
        }

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("First batch"));
        assert!(content.contains("Second batch"));
    }
}
