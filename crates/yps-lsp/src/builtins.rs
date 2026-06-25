#[must_use]
pub fn builtin_doc(name: &str) -> Option<&'static str> {
    match name {
        "сказать" => Some("**console.log** — вывести аргументы в консоль"),
        "длина" => Some("**length** — длина строки, массива или типизированного массива"),
        "тип" => Some("**typeof** — имя типа значения строкой"),
        "число" => Some("**Number(x)** — привести значение к числу"),
        "БигЦелое" => Some("**BigInt(x)** — привести значение к большому целому"),
        "строка" => Some("**String(x)** — привести значение к строке"),
        "втолкнуть" => Some("**Array.prototype.push** — добавить значение в массив, вернуть массив"),
        "Косяк" => Some("**Error** — создать объект ошибки"),
        "этоКосяк" => Some("**x instanceof Error** — проверить, что значение является ошибкой"),
        "RegExp" => Some("**RegExp** — создать регулярное выражение"),
        "Дата" => Some("**Date** — создать объект даты/времени"),
        "чутка" => Some("**setTimeout** — вызвать колбэк один раз через N мс"),
        "отменаЧутки" => Some("**clearTimeout** — отменить отложенный вызов"),
        "интервал" => Some("**setInterval** — вызывать колбэк периодически каждые N мс"),
        "отменаИнтервала" => Some("**clearInterval** — отменить периодический вызов"),
        "сразу" => Some("**setImmediate** — выполнить колбэк после текущего стека вызовов"),
        "наСледующемТике" => {
            Some("**queueMicrotask / process.nextTick** — поставить колбэк в очередь микрозадач")
        }
        "подождать" => Some("**Promise(r => setTimeout(r, мс))** — промис, разрешающийся через N мс"),
        "сОчередить" => Some("**queueMicrotask** — поставить колбэк в микроочередь, вернуть промис"),
        "прочестьСтроку" => Some("**readline** — прочитать одну строку из stdin"),
        "прочестьВсё" => Some("**read all stdin** — прочитать весь ввод из stdin"),
        "сказать.ошибка" => Some("**console.error** — вывести сообщение об ошибке"),
        "сказать.предупреждение" => Some("**console.warn** — вывести предупреждение"),
        "сказать.инфо" => Some("**console.info** — вывести информационное сообщение"),
        "сказать.отладка" => Some("**console.debug** — вывести отладочное сообщение"),
        "сказать.таблица" => Some("**console.table** — вывести данные таблицей"),
        "сказать.время" => Some("**console.time** — запустить таймер с меткой"),
        "сказать.времяСтоп" => Some("**console.timeEnd** — остановить таймер и вывести время"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yps_interpreter::builtins::builtin_names;

    #[test]
    fn every_builtin_has_a_doc() {
        let missing: Vec<&str> = builtin_names().iter().copied().filter(|n| builtin_doc(n).is_none()).collect();
        assert!(missing.is_empty(), "builtins without docs: {missing:?}");
    }

    #[test]
    fn doc_mentions_js_equivalent() {
        assert!(builtin_doc("сказать").unwrap().contains("console.log"));
        assert!(builtin_doc("тип").unwrap().contains("typeof"));
        assert!(builtin_doc("сказать.ошибка").unwrap().contains("console.error"));
    }

    #[test]
    fn unknown_name_has_no_doc() {
        assert!(builtin_doc("неизвестно").is_none());
    }
}
