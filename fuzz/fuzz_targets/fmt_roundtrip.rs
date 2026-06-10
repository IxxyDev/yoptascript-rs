#![no_main]

use libfuzzer_sys::fuzz_target;
use yps_fmt::format_source;

fuzz_target!(|data: &str| {
    let Ok(first) = format_source(data) else { return };
    match format_source(&first.text) {
        Ok(second) => {
            assert!(second.already_formatted, "fmt не идемпотентен: повторный прогон снова меняет текст");
            assert_eq!(second.text, first.text, "fmt не идемпотентен: вывод второго прогона отличается");
        }
        Err(e) => panic!("отформатированный вывод не форматируется повторно: {e}"),
    }
});
