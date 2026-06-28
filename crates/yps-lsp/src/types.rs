use tower_lsp::lsp_types::{CompletionItem, CompletionItemKind, Documentation, MarkupContent, MarkupKind};

#[derive(Clone, Copy)]
pub struct Member {
    pub ru: &'static str,
    pub js: &'static str,
    pub desc: &'static str,
    pub is_property: bool,
}

const fn meth(ru: &'static str, js: &'static str, desc: &'static str) -> Member {
    Member { ru, js, desc, is_property: false }
}

const fn prop(ru: &'static str, js: &'static str, desc: &'static str) -> Member {
    Member { ru, js, desc, is_property: true }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Surface {
    Global,
    Receiver,
}

pub struct BuiltinType {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub js: &'static str,
    pub kind: CompletionItemKind,
    pub surface: Surface,
    pub desc: &'static str,
    pub members: &'static [Member],
}

impl BuiltinType {
    fn matches(&self, word: &str) -> bool {
        self.name == word || self.aliases.contains(&word)
    }
}

const TYPES: &[BuiltinType] = &[
    BuiltinType {
        name: "Матан",
        aliases: &["Math"],
        js: "Math",
        kind: CompletionItemKind::MODULE,
        surface: Surface::Global,
        desc: "математические константы и функции",
        members: &[
            prop("ПИ", "PI", "число π"),
            prop("Е", "E", "число Эйлера e"),
            meth("пол", "floor", "округление вниз"),
            meth("потолок", "ceil", "округление вверх"),
            meth("округлить", "round", "округление к ближайшему"),
            meth("модуль", "abs", "модуль числа"),
            meth("мин", "min", "минимум из аргументов"),
            meth("макс", "max", "максимум из аргументов"),
            meth("степень", "pow", "возведение в степень"),
            meth("корень", "sqrt", "квадратный корень"),
            meth("рандом", "random", "случайное число [0, 1)"),
            meth("знак", "sign", "знак числа"),
            meth("обрезать", "trunc", "отбросить дробную часть"),
            meth("лог", "log", "натуральный логарифм"),
            meth("синус", "sin", "синус"),
            meth("косинус", "cos", "косинус"),
            meth("тангенс", "tan", "тангенс"),
        ],
    },
    BuiltinType {
        name: "Кент",
        aliases: &["Object"],
        js: "Object",
        kind: CompletionItemKind::MODULE,
        surface: Surface::Global,
        desc: "статические методы для работы с объектами",
        members: &[
            meth("ключи", "keys", "массив ключей объекта"),
            meth("значения", "values", "массив значений объекта"),
            meth("записи", "entries", "массив пар [ключ, значение]"),
            meth("назначить", "assign", "скопировать свойства в цель"),
            meth("имеетСвоё", "hasOwn", "есть ли собственное свойство"),
            meth("изЗаписей", "fromEntries", "объект из пар [ключ, значение]"),
            meth("группировать", "groupBy", "сгруппировать по ключу"),
            meth("создать", "create", "объект с заданным прототипом"),
            meth("прототип", "getPrototypeOf", "прототип объекта"),
            meth("назначитьПрототип", "setPrototypeOf", "задать прототип"),
            meth("заморозить", "freeze", "запретить изменения объекта"),
            meth("заморожен", "isFrozen", "заморожен ли объект"),
        ],
    },
    BuiltinType {
        name: "Жсон",
        aliases: &["JSON"],
        js: "JSON",
        kind: CompletionItemKind::MODULE,
        surface: Surface::Global,
        desc: "сериализация и разбор JSON",
        members: &[
            meth("разобрать", "parse", "разобрать строку JSON в значение"),
            meth("вСтроку", "stringify", "сериализовать значение в JSON"),
        ],
    },
    BuiltinType {
        name: "Помойка",
        aliases: &["Array"],
        js: "Array",
        kind: CompletionItemKind::MODULE,
        surface: Surface::Global,
        desc: "статические методы конструктора массива",
        members: &[
            meth("являетсяПомойкой", "isArray", "является ли значение массивом"),
            meth("извне", "from", "массив из итерируемого/массивоподобного"),
            meth("нового", "of", "массив из переданных аргументов"),
        ],
    },
    BuiltinType {
        name: "Хуйня",
        aliases: &["Number"],
        js: "Number",
        kind: CompletionItemKind::MODULE,
        surface: Surface::Global,
        desc: "статические методы конструктора числа",
        members: &[
            meth("конечна", "isFinite", "конечное ли число"),
            meth("целая", "isInteger", "целое ли число"),
            meth("нихуя", "isNaN", "является ли NaN"),
            meth("разобратьЦелое", "parseInt", "разобрать целое из строки"),
            meth("разобратьЧисло", "parseFloat", "разобрать число из строки"),
        ],
    },
    BuiltinType {
        name: "Отражение",
        aliases: &["Reflect"],
        js: "Reflect",
        kind: CompletionItemKind::MODULE,
        surface: Surface::Global,
        desc: "низкоуровневые операции над объектами",
        members: &[
            meth("получить", "get", "прочитать свойство"),
            meth("есть", "has", "есть ли свойство"),
            meth("прототипОт", "getPrototypeOf", "прототип объекта"),
            meth("собственныеКлючи", "ownKeys", "собственные ключи объекта"),
            meth("применить", "apply", "вызвать функцию с массивом аргументов"),
            meth("построить", "construct", "создать экземпляр конструктором"),
        ],
    },
    BuiltinType {
        name: "Итератор",
        aliases: &["Iterator"],
        js: "Iterator",
        kind: CompletionItemKind::CLASS,
        surface: Surface::Global,
        desc: "хелперы итераторов",
        members: &[
            meth("от", "from", "обернуть итерируемое в итератор-хелпер"),
            meth("склеить", "concat", "склеить несколько итераторов"),
        ],
    },
    BuiltinType {
        name: "Карта",
        aliases: &["Map"],
        js: "Map",
        kind: CompletionItemKind::CLASS,
        surface: Surface::Global,
        desc: "коллекция пар ключ-значение (new)",
        members: &[],
    },
    BuiltinType {
        name: "Набор",
        aliases: &["Set"],
        js: "Set",
        kind: CompletionItemKind::CLASS,
        surface: Surface::Global,
        desc: "коллекция уникальных значений (new)",
        members: &[],
    },
    BuiltinType {
        name: "СлабаяКарта",
        aliases: &["WeakMap"],
        js: "WeakMap",
        kind: CompletionItemKind::CLASS,
        surface: Surface::Global,
        desc: "карта со слабыми ссылками на ключи (new)",
        members: &[],
    },
    BuiltinType {
        name: "СлабыйНабор",
        aliases: &["WeakSet"],
        js: "WeakSet",
        kind: CompletionItemKind::CLASS,
        surface: Surface::Global,
        desc: "набор со слабыми ссылками (new)",
        members: &[],
    },
    BuiltinType {
        name: "СлабаяСсылка",
        aliases: &["WeakRef"],
        js: "WeakRef",
        kind: CompletionItemKind::CLASS,
        surface: Surface::Global,
        desc: "слабая ссылка на объект (new)",
        members: &[],
    },
    BuiltinType {
        name: "РеестрФинализации",
        aliases: &["FinalizationRegistry"],
        js: "FinalizationRegistry",
        kind: CompletionItemKind::CLASS,
        surface: Surface::Global,
        desc: "реестр колбэков финализации (new)",
        members: &[],
    },
    BuiltinType {
        name: "Симбол",
        aliases: &["Symbol"],
        js: "Symbol",
        kind: CompletionItemKind::CLASS,
        surface: Surface::Global,
        desc: "уникальные символы и общеизвестные символы",
        members: &[
            prop("итератор", "iterator", "общеизвестный символ Symbol.iterator"),
            prop("асинхИтератор", "asyncIterator", "Symbol.asyncIterator"),
            prop("вПримитив", "toPrimitive", "Symbol.toPrimitive"),
            prop("строковыйТег", "toStringTag", "Symbol.toStringTag"),
            meth("для", "for", "символ из глобального реестра по ключу"),
            meth("ключДля", "keyFor", "ключ символа из реестра"),
        ],
    },
    BuiltinType {
        name: "Дата",
        aliases: &["Date"],
        js: "Date",
        kind: CompletionItemKind::CLASS,
        surface: Surface::Global,
        desc: "дата и время (new)",
        members: &[meth("сейчас", "now", "текущее время в миллисекундах")],
    },
    BuiltinType {
        name: "СловоПацана",
        aliases: &["Promise"],
        js: "Promise",
        kind: CompletionItemKind::CLASS,
        surface: Surface::Global,
        desc: "промис — асинхронное значение (new)",
        members: &[
            meth("решить", "resolve", "промис, разрешённый значением"),
            meth("отвергнуть", "reject", "промис, отклонённый причиной"),
            meth("всех", "all", "ждать все промисы"),
            meth("всехУстаканить", "allSettled", "ждать завершения всех"),
            meth("любой", "any", "первый успешный промис"),
            meth("гонка", "race", "первый завершившийся промис"),
            meth("отПодождать", "whenAborted", "промис, разрешающийся при срабатывании сигнала отмены"),
            meth("сРешалками", "withResolvers", "промис с внешними resolve/reject"),
            meth("попробовать", "try", "обернуть вызов в промис"),
        ],
    },
    BuiltinType {
        name: "КонтроллёрОтмены",
        aliases: &["AbortController"],
        js: "AbortController",
        kind: CompletionItemKind::CLASS,
        surface: Surface::Global,
        desc: "контроллёр сигнала отмены (new)",
        members: &[],
    },
    BuiltinType {
        name: "СигналОтмены",
        aliases: &["AbortSignal"],
        js: "AbortSignal",
        kind: CompletionItemKind::CLASS,
        surface: Surface::Global,
        desc: "сигнал отмены операций",
        members: &[
            meth("любой", "any", "сигнал, срабатывающий по любому из переданных"),
            meth("отВремени", "timeout", "сигнал, срабатывающий через N мс"),
        ],
    },
    BuiltinType {
        name: "Посредник",
        aliases: &["Proxy"],
        js: "Proxy",
        kind: CompletionItemKind::CLASS,
        surface: Surface::Global,
        desc: "перехват операций над объектом (new)",
        members: &[],
    },
    BuiltinType {
        name: "ОбластьБайтов",
        aliases: &["ArrayBuffer"],
        js: "ArrayBuffer",
        kind: CompletionItemKind::CLASS,
        surface: Surface::Global,
        desc: "буфер сырых байтов (new)",
        members: &[],
    },
    BuiltinType {
        name: "ОбзорБайтов",
        aliases: &["DataView"],
        js: "DataView",
        kind: CompletionItemKind::CLASS,
        surface: Surface::Global,
        desc: "типизированный доступ к буферу байтов (new)",
        members: &[],
    },
    BuiltinType {
        name: "ФС",
        aliases: &["fs"],
        js: "fs",
        kind: CompletionItemKind::MODULE,
        surface: Surface::Global,
        desc: "файловая система",
        members: &[
            meth("прочитать", "readFileSync", "прочитать файл"),
            meth("записать", "writeFileSync", "записать файл"),
            meth("дописать", "appendFileSync", "дописать в файл"),
            meth("удалить", "unlinkSync", "удалить файл"),
            meth("существует", "existsSync", "существует ли путь"),
            meth("этоПапка", "isDirectory", "путь — это папка"),
            meth("этоФайл", "isFile", "путь — это файл"),
            meth("список", "readdirSync", "список содержимого папки"),
            meth("создатьПапку", "mkdirSync", "создать папку"),
            meth("удалитьПапку", "rmdirSync", "удалить папку"),
        ],
    },
    BuiltinType {
        name: "Процесс",
        aliases: &["process"],
        js: "process",
        kind: CompletionItemKind::MODULE,
        surface: Surface::Global,
        desc: "текущий процесс",
        members: &[
            meth("выход", "exit", "завершить процесс с кодом"),
            meth("сменитьПапку", "chdir", "сменить рабочую папку"),
            prop("перем", "env", "переменные окружения"),
        ],
    },
    BuiltinType {
        name: "Сеть",
        aliases: &["net"],
        js: "net",
        kind: CompletionItemKind::MODULE,
        surface: Surface::Global,
        desc: "сетевые запросы",
        members: &[meth("достать", "fetch", "выполнить HTTP-запрос")],
    },
    BuiltinType {
        name: "Строка",
        aliases: &[],
        js: "String",
        kind: CompletionItemKind::CLASS,
        surface: Surface::Receiver,
        desc: "методы строки",
        members: &[
            prop("длина", "length", "длина строки"),
            meth("символВ", "charAt", "символ по индексу"),
            meth("кодСимволаВ", "charCodeAt", "код символа по индексу"),
            meth("найтиПодстроку", "indexOf", "индекс первого вхождения"),
            meth("найтиПодстрокуСконца", "lastIndexOf", "индекс последнего вхождения"),
            meth("содержит", "includes", "содержит ли подстроку"),
            meth("отрезать", "slice", "подстрока по индексам"),
            meth("подстрока", "substring", "подстрока между индексами"),
            meth("вВерхнийРегистр", "toUpperCase", "в верхний регистр"),
            meth("вНижнийРегистр", "toLowerCase", "в нижний регистр"),
            meth("обрезать", "trim", "убрать пробелы по краям"),
            meth("обрезатьСлева", "trimStart", "убрать пробелы слева"),
            meth("обрезатьСправа", "trimEnd", "убрать пробелы справа"),
            meth("разбить", "split", "разбить строку на массив"),
            meth("заменить", "replace", "заменить первое совпадение"),
            meth("заменитьВсе", "replaceAll", "заменить все совпадения"),
            meth("совпадает", "match", "совпадение с регуляркой"),
            meth("найтиВсе", "matchAll", "все совпадения с регуляркой"),
            meth("найтиИндекс", "search", "индекс совпадения с регуляркой"),
            meth("начинаетсяС", "startsWith", "начинается ли с подстроки"),
            meth("заканчиваетсяНа", "endsWith", "заканчивается ли подстрокой"),
            meth("повторить", "repeat", "повторить строку N раз"),
            meth("дополнитьСлева", "padStart", "дополнить слева до длины"),
            meth("дополнитьСправа", "padEnd", "дополнить справа до длины"),
            meth("поИндексу", "at", "символ по индексу (можно отрицательный)"),
        ],
    },
    BuiltinType {
        name: "Массив",
        aliases: &[],
        js: "Array",
        kind: CompletionItemKind::CLASS,
        surface: Surface::Receiver,
        desc: "методы массива",
        members: &[
            prop("длина", "length", "длина массива"),
            meth("добавить", "push", "добавить в конец"),
            meth("вытолкнуть", "pop", "снять с конца"),
            meth("снять", "shift", "снять с начала"),
            meth("подсунуть", "unshift", "добавить в начало"),
            meth("отрезать", "slice", "копия среза массива"),
            meth("найтиИндекс", "indexOf", "индекс первого вхождения"),
            meth("найтиПоследнийПо", "lastIndexOf", "индекс последнего вхождения"),
            meth("включает", "includes", "содержит ли элемент"),
            meth("склеить", "join", "объединить в строку"),
            meth("перевернуть", "reverse", "перевернуть на месте"),
            meth("склеитьМассивы", "concat", "склеить массивы"),
            meth("сортировать", "sort", "отсортировать на месте"),
            meth("преобразовать", "map", "преобразовать каждый элемент"),
            meth("отфильтровать", "filter", "оставить подходящие"),
            meth("свернуть", "reduce", "свернуть к одному значению"),
            meth("свернутьСправа", "reduceRight", "свернуть справа налево"),
            meth("каждый", "forEach", "выполнить для каждого"),
            meth("найти", "find", "первый подходящий элемент"),
            meth("найтиИндексПо", "findIndex", "индекс первого подходящего"),
            meth("некоторые", "some", "есть ли хоть один подходящий"),
            meth("все", "every", "все ли подходят"),
            meth("поИндексу", "at", "элемент по индексу"),
            meth("плоский", "flat", "развернуть вложенные массивы"),
            meth("плоскоПреобразовать", "flatMap", "map + flat"),
            meth("найтиПоследний", "findLast", "последний подходящий"),
            meth("найтиПоследнийИндекс", "findLastIndex", "индекс последнего подходящего"),
            meth("перевёрнутый", "toReversed", "перевёрнутая копия"),
            meth("отсортированный", "toSorted", "отсортированная копия"),
            meth("вырезать", "splice", "удалить/вставить на месте"),
            meth("вырезанный", "toSpliced", "splice без мутации"),
            meth("сЗаменой", "with", "копия с заменой по индексу"),
        ],
    },
    BuiltinType {
        name: "Число",
        aliases: &[],
        js: "Number",
        kind: CompletionItemKind::CLASS,
        surface: Surface::Receiver,
        desc: "методы числа",
        members: &[
            meth("вСтроку", "toString", "число в строку"),
            meth("фиксированный", "toFixed", "строка с N знаками после точки"),
        ],
    },
    BuiltinType {
        name: "Карта (экземпляр)",
        aliases: &[],
        js: "Map",
        kind: CompletionItemKind::CLASS,
        surface: Surface::Receiver,
        desc: "методы экземпляра карты",
        members: &[
            prop("размер", "size", "число записей"),
            meth("поставить", "set", "записать пару ключ-значение"),
            meth("взять", "get", "значение по ключу"),
            meth("имеет", "has", "есть ли ключ"),
            meth("удалить", "delete", "удалить ключ"),
            meth("очистить", "clear", "очистить карту"),
            meth("ключи", "keys", "итератор ключей"),
            meth("значения", "values", "итератор значений"),
            meth("записи", "entries", "итератор записей"),
            meth("взятьИлиВставить", "getOrInsert", "взять или вставить значение"),
            meth("взятьИлиВычислить", "getOrInsertComputed", "взять или вычислить значение"),
            meth("каждый", "forEach", "выполнить для каждой записи"),
        ],
    },
    BuiltinType {
        name: "Набор (экземпляр)",
        aliases: &[],
        js: "Set",
        kind: CompletionItemKind::CLASS,
        surface: Surface::Receiver,
        desc: "методы экземпляра набора",
        members: &[
            prop("размер", "size", "число элементов"),
            meth("добавить", "add", "добавить элемент"),
            meth("имеет", "has", "есть ли элемент"),
            meth("удалить", "delete", "удалить элемент"),
            meth("очистить", "clear", "очистить набор"),
            meth("значения", "values", "итератор значений"),
            meth("каждый", "forEach", "выполнить для каждого"),
            meth("объединение", "union", "объединение наборов"),
            meth("пересечение", "intersection", "пересечение наборов"),
            meth("разница", "difference", "разность наборов"),
            meth("симметричнаяРазница", "symmetricDifference", "симметрическая разность"),
            meth("подмножествоОт", "isSubsetOf", "является ли подмножеством"),
            meth("надмножествоОт", "isSupersetOf", "является ли надмножеством"),
            meth("непересекаетсяС", "isDisjointFrom", "не пересекается ли"),
        ],
    },
    BuiltinType {
        name: "Дата (экземпляр)",
        aliases: &[],
        js: "Date",
        kind: CompletionItemKind::CLASS,
        surface: Surface::Receiver,
        desc: "методы экземпляра даты",
        members: &[
            meth("времяМс", "getTime", "время в миллисекундах"),
            meth("вЧисло", "valueOf", "время числом"),
            meth("год", "getFullYear", "год"),
            meth("месяц", "getMonth", "месяц (0–11)"),
            meth("день", "getDate", "день месяца"),
            meth("деньНедели", "getDay", "день недели"),
            meth("часы", "getHours", "часы"),
            meth("минуты", "getMinutes", "минуты"),
            meth("секунды", "getSeconds", "секунды"),
            meth("миллисекунды", "getMilliseconds", "миллисекунды"),
            meth("вИСО", "toISOString", "строка в формате ISO"),
            meth("вСтроку", "toString", "строковое представление"),
        ],
    },
    BuiltinType {
        name: "Регулярка",
        aliases: &[],
        js: "RegExp",
        kind: CompletionItemKind::CLASS,
        surface: Surface::Receiver,
        desc: "методы регулярного выражения",
        members: &[
            meth("проверить", "test", "проверить совпадение"),
            meth("найти", "exec", "найти совпадение с группами"),
            meth("вСтроку", "toString", "строковое представление"),
            prop("источник", "source", "исходный шаблон"),
            prop("флаги", "flags", "строка флагов"),
            prop("последнийИндекс", "lastIndex", "индекс для следующего поиска"),
        ],
    },
    BuiltinType {
        name: "Итератор (экземпляр)",
        aliases: &[],
        js: "Iterator",
        kind: CompletionItemKind::CLASS,
        surface: Surface::Receiver,
        desc: "методы итератора",
        members: &[
            meth("следующий", "next", "следующий элемент"),
            meth("вернуть", "return", "завершить итератор"),
            meth("кинуть", "throw", "бросить в итератор"),
            meth("преобразовать", "map", "ленивое преобразование"),
            meth("отфильтровать", "filter", "ленивая фильтрация"),
            meth("взять", "take", "взять первые N"),
            meth("пропустить", "drop", "пропустить первые N"),
            meth("вМассив", "toArray", "собрать в массив"),
            meth("каждый", "forEach", "выполнить для каждого"),
            meth("свернуть", "reduce", "свернуть к значению"),
            meth("некоторые", "some", "есть ли подходящий"),
            meth("все", "every", "все ли подходят"),
            meth("найти", "find", "первый подходящий"),
        ],
    },
    BuiltinType {
        name: "СловоПацана (экземпляр)",
        aliases: &[],
        js: "Promise",
        kind: CompletionItemKind::CLASS,
        surface: Surface::Receiver,
        desc: "методы экземпляра промиса",
        members: &[
            meth("потом", "then", "обработать результат"),
            meth("ловить", "catch", "обработать ошибку"),
            meth("наконец", "finally", "выполнить в любом случае"),
        ],
    },
];

fn markdown(value: String) -> Documentation {
    Documentation::MarkupContent(MarkupContent { kind: MarkupKind::Markdown, value })
}

fn member_kind(m: &Member) -> CompletionItemKind {
    if m.is_property { CompletionItemKind::PROPERTY } else { CompletionItemKind::METHOD }
}

#[must_use]
pub fn type_doc(word: &str) -> Option<String> {
    let ty = TYPES.iter().find(|t| t.surface == Surface::Global && t.matches(word))?;
    Some(format!("**{}** — {}", ty.js, ty.desc))
}

#[must_use]
pub fn member_doc(word: &str) -> Option<String> {
    let mut lines: Vec<String> = Vec::new();
    for ty in TYPES {
        for m in ty.members {
            if m.ru == word || m.js == word {
                lines.push(format!("**{}.{}** — {} *(JS: {})*", ty.js, m.ru, m.desc, m.js));
            }
        }
    }
    if lines.is_empty() { None } else { Some(lines.join("\n\n")) }
}

#[must_use]
pub fn is_known_global(word: &str) -> bool {
    TYPES.iter().any(|t| t.surface == Surface::Global && t.matches(word))
}

#[must_use]
pub fn global_type_items() -> Vec<CompletionItem> {
    TYPES
        .iter()
        .filter(|t| t.surface == Surface::Global)
        .map(|t| CompletionItem {
            label: t.name.to_string(),
            kind: Some(t.kind),
            detail: Some(t.js.to_string()),
            documentation: Some(markdown(format!("**{}** — {}", t.js, t.desc))),
            ..Default::default()
        })
        .collect()
}

#[must_use]
pub fn member_items_for(receiver: Option<&str>) -> Vec<CompletionItem> {
    let item = |ty: &BuiltinType, m: &Member| CompletionItem {
        label: m.ru.to_string(),
        kind: Some(member_kind(m)),
        detail: Some(format!("{}.{}", ty.js, m.js)),
        documentation: Some(markdown(format!("**{}.{}** — {} *(JS: {})*", ty.js, m.ru, m.desc, m.js))),
        ..Default::default()
    };

    if let Some(recv) = receiver
        && !recv.is_empty()
        && let Some(ty) = TYPES.iter().find(|t| t.surface == Surface::Global && t.matches(recv))
    {
        return ty.members.iter().map(|m| item(ty, m)).collect();
    }

    let mut items = Vec::new();
    let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for ty in TYPES {
        for m in ty.members {
            if seen.insert(m.ru) {
                items.push(item(ty, m));
            }
        }
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;
    use yps_interpreter::builtins::builtin_names;
    use yps_interpreter::stdlib::build_globals;

    #[test]
    fn global_types_are_real_runtime_globals() {
        let globals: std::collections::HashSet<String> = build_globals().into_iter().map(|(name, _)| name).collect();
        let builtins: std::collections::HashSet<&str> = builtin_names().iter().copied().collect();
        for ty in TYPES.iter().filter(|t| t.surface == Surface::Global) {
            assert!(
                globals.contains(ty.name) || builtins.contains(ty.name),
                "тип '{}' каталога не зарегистрирован как глобал интерпретатора",
                ty.name
            );
        }
    }

    #[test]
    fn type_doc_resolves_namespaces() {
        assert!(type_doc("Матан").unwrap().contains("Math"));
        assert!(type_doc("Жсон").unwrap().contains("JSON"));
        assert!(type_doc("Карта").unwrap().contains("Map"));
        assert!(type_doc("неизвестно").is_none());
    }

    #[test]
    fn member_doc_resolves_and_lists_owners() {
        assert!(member_doc("пол").unwrap().contains("Math.пол"));
        let foreach = member_doc("каждый").unwrap();
        assert!(foreach.contains("forEach"));
    }

    #[test]
    fn member_items_for_namespace_are_scoped() {
        let items = member_items_for(Some("Матан"));
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"пол"));
        assert!(labels.contains(&"корень"));
        assert!(!labels.contains(&"добавить"), "методы массива не должны попадать в Матан");
    }

    #[test]
    fn member_items_union_when_receiver_unknown() {
        let items = member_items_for(None);
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"вВерхнийРегистр"));
        assert!(labels.contains(&"добавить"));
    }
}
