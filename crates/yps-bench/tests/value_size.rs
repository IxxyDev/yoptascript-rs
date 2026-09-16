use std::mem::size_of;

const INTERPRETER_VALUE_LIMIT: usize = 32;
const VM_VALUE_LIMIT: usize = 64;

#[test]
fn interpreter_value_stays_small() {
    let actual = size_of::<yps_interpreter::Value>();
    assert!(
        actual <= INTERPRETER_VALUE_LIMIT,
        "yps_interpreter::Value вырос до {actual} байт (лимит {INTERPRETER_VALUE_LIMIT})"
    );
}

#[test]
fn vm_value_stays_small() {
    let actual = size_of::<yps_vm::Value>();
    assert!(actual <= VM_VALUE_LIMIT, "yps_vm::Value вырос до {actual} байт (лимит {VM_VALUE_LIMIT})");
}
