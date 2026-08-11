//! Debug Adapter Protocol server for YoptaScript, driving the tree-walking interpreter
//! (`yps-interpreter`) through its additive `DebugHook`.
//!
//! Deliberate limitations:
//! - Stepping granularity is one statement, not one expression: the hook fires once per `Stmt`.
//! - Debuggee stdin is blocked: `прочестьСтроку`/`прочестьВсё` raise a catchable runtime error,
//!   because the adapter's stdin carries the Content-Length-framed protocol stream itself and a
//!   debuggee read would steal protocol bytes. Debuggee stdout/stderr (`сказать`/`сказать.*`) are
//!   captured via the interpreter's `OutputSink` and forwarded as DAP `output` events; both the
//!   sink and the stdin block are inherited by imported modules, which run in a sub-interpreter.
//! - Uncaught errors inside timer/promise callbacks are printed to the adapter's stderr (not
//!   through the sink), so they never corrupt the protocol stream but also don't reach the
//!   client's debug console.
//! - Breakpoints are accepted only for the launched file: `setBreakpoints` for any other path
//!   answers with unverified breakpoints instead of resolving lines against the wrong source.
//! - Only the innermost stack frame exposes real locals; the interpreter keeps no per-frame
//!   environment snapshots, so outer frames report an empty `Locals` scope.
//! - Variables are rendered flat (no expandable children for objects, arrays or maps).
//! - A module-level variable that shadows a builtin name (e.g. `гыы длина = 99;`) is invisible in
//!   `variables`: top-level script bindings share the same `EnvFrame` as builtins, so the debugger
//!   filters out anything present in the pre-run global snapshot to avoid listing every builtin.
//! - VS Code editor wiring (`contributes.debuggers`, adapter factory) lives in `editors/vscode`,
//!   not in this crate.

pub mod breakpoints;
pub mod debuggee;
pub mod protocol;
pub mod session;

use std::io::{self, BufReader, Read, Write};

use session::{Incoming, Session};

/// Runs one DAP session: `input` is read on a helper thread so that debuggee events and client
/// requests land on the same queue, which is what makes an asynchronous `pause` possible.
pub fn serve<R: Read + Send + 'static, W: Write>(input: R, output: &mut W) -> io::Result<()> {
    let (tx, rx) = std::sync::mpsc::channel();
    let mut session = Session::new(tx.clone());

    std::thread::spawn(move || {
        let mut reader = BufReader::new(input);
        loop {
            match protocol::read_message(&mut reader) {
                Ok(Some(message)) => {
                    if tx.send(Incoming::Client(message)).is_err() {
                        return;
                    }
                }
                _ => {
                    let _ = tx.send(Incoming::ClientEof);
                    return;
                }
            }
        }
    });

    while let Ok(incoming) = rx.recv() {
        for message in session.handle(incoming) {
            protocol::write_message(output, &message)?;
        }
        if session.should_exit() {
            break;
        }
    }
    Ok(())
}
