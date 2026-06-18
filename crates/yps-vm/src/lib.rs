use std::cell::RefCell;
use std::io::{self, Write};
use std::rc::Rc;

use yps_parser::ast::Program;

mod builtins;
pub mod chunk;
pub mod compiler;
pub mod error;
pub mod value;
pub mod vm;

#[cfg(test)]
mod tests;

pub use chunk::{FnProto, disassemble};
pub use compiler::compile_program;
pub use error::{CompileError, ExecError, VmError};
pub use value::Value;
pub use vm::Vm;

pub fn execute(program: &Program) -> Result<(), ExecError> {
    let proto = compile_program(program)?;
    let mut vm = Vm::new();
    vm.run(proto)?;
    Ok(())
}

pub fn run_to_string(program: &Program) -> Result<String, ExecError> {
    let proto = compile_program(program)?;
    let buf: Rc<RefCell<Vec<u8>>> = Rc::new(RefCell::new(Vec::new()));
    let mut vm = Vm::with_writer(Box::new(SharedWriter(Rc::clone(&buf))));
    let result = vm.run(proto);
    drop(vm);
    let bytes = buf.borrow().clone();
    result?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

struct SharedWriter(Rc<RefCell<Vec<u8>>>);

impl Write for SharedWriter {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(b);
        Ok(b.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
