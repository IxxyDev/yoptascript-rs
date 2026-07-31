use std::cell::RefCell;
use std::io::Write;
use std::rc::Rc;

pub trait OutputSink {
    fn write_line(&mut self, line: &str);

    fn write_error_line(&mut self, line: &str) {
        self.write_line(line);
    }
}

pub struct StdoutSink;

impl OutputSink for StdoutSink {
    fn write_line(&mut self, line: &str) {
        println!("{line}");
    }

    fn write_error_line(&mut self, line: &str) {
        let mut err = std::io::stderr().lock();
        let _ = writeln!(err, "{line}");
    }
}

/// Buffer shared between the sink installed on the interpreter and the caller that reads it
/// back after `run` returns; the interpreter owns its sink by value, so a handle is the only
/// way for a host (playground, tests) to see what was written.
#[derive(Clone, Default)]
pub struct BufferSink {
    buffer: Rc<RefCell<String>>,
}

impl BufferSink {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn take(&self) -> String {
        std::mem::take(&mut *self.buffer.borrow_mut())
    }
}

impl OutputSink for BufferSink {
    fn write_line(&mut self, line: &str) {
        let mut buf = self.buffer.borrow_mut();
        buf.push_str(line);
        buf.push('\n');
    }
}
