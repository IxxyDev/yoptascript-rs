#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Helper {
    Typeof,
    Len,
    Push,
    IsError,
    Sleep,
    ReadLine,
    ReadAll,
    Round,
    Hypot,
}

impl Helper {
    pub(crate) const fn js_name(self) -> &'static str {
        match self {
            Self::Typeof => "__ypsTypeof",
            Self::Len => "__ypsLen",
            Self::Push => "__ypsPush",
            Self::IsError => "__ypsIsError",
            Self::Sleep => "__ypsSleep",
            Self::ReadLine => "__ypsReadLine",
            Self::ReadAll => "__ypsReadAll",
            Self::Round => "__ypsRound",
            Self::Hypot => "__ypsHypot",
        }
    }

    pub(crate) const fn source(self) -> &'static str {
        match self {
            Self::Typeof => TYPEOF_SRC,
            Self::Len => LEN_SRC,
            Self::Push => PUSH_SRC,
            Self::IsError => IS_ERROR_SRC,
            Self::Sleep => SLEEP_SRC,
            Self::ReadLine => READ_LINE_SRC,
            Self::ReadAll => READ_ALL_SRC,
            Self::Round => ROUND_SRC,
            Self::Hypot => HYPOT_SRC,
        }
    }

    pub(crate) const fn needs_stdin(self) -> bool {
        matches!(self, Self::ReadLine | Self::ReadAll)
    }
}

/// Зеркалит `Value::type_name()` интерпретатора для тех вариантов, которые вообще
/// достижимы из транспилируемого подмножества. Best-effort и потому неточны:
/// `карта`/`набор`/`слабая*`/`символ` — соответствующие глобалы (`Карта`, `Набор`,
/// `Симбол`, ...) объявлены неподдерживаемыми, так что эти ветки срабатывают только на
/// значениях из внешнего JS; `обещание`/`дата`/`регэксп` опираются на `instanceof` и
/// промахнутся на объектах из другого realm. Варианты `ОбластьБайтов`/`ОбзорБайтов`/
/// типизированные массивы/`посредник`/`контроллёрОтмены` и внутренние продолжения
/// рантайма не покрываются вовсе — их нельзя создать без запрещённых глобалов.
const TYPEOF_SRC: &str = r#"function __ypsTypeof(v) {
  if (v === null) return "нулл";
  if (v === undefined) return "неопределено";
  if (Array.isArray(v)) return "массив";
  switch (typeof v) {
    case "number": return "число";
    case "bigint": return "бигцелое";
    case "string": return "строка";
    case "boolean": return "булево";
    case "symbol": return "символ";
    case "function":
      return /^\s*class[\s{]/.test(Function.prototype.toString.call(v)) ? "класс" : "функция";
  }
  const tag = v[Symbol.toStringTag];
  if (tag === "Generator" || tag === "AsyncGenerator") return "итератор";
  if (v instanceof Map) return "карта";
  if (v instanceof Set) return "набор";
  if (v instanceof WeakMap) return "слабаяКарта";
  if (v instanceof WeakSet) return "слабыйНабор";
  if (v instanceof WeakRef) return "слабаяСсылка";
  if (v instanceof RegExp) return "регэксп";
  if (v instanceof Date) return "дата";
  if (v instanceof Promise) return "обещание";
  return "объект";
}"#;

const LEN_SRC: &str = r"function __ypsLen(v) {
  return v.length;
}";

const PUSH_SRC: &str = r"function __ypsPush(arr, val) {
  arr.push(val);
  return arr;
}";

const IS_ERROR_SRC: &str = r"function __ypsIsError(v) {
  return v instanceof Error;
}";

const SLEEP_SRC: &str = r"function __ypsSleep(ms) {
  return new Promise((r) => setTimeout(r, ms));
}";

/// `Math.round` в JS округляет половины к +бесконечности (`Math.round(-1.5) === -1`),
/// а `f64::round` в интерпретаторе — от нуля (`(-1.5).round() == -2.0`). Шим повторяет
/// семантику интерпретатора: знак отдельно, модуль округляется вверх на половине.
const ROUND_SRC: &str = r"function __ypsRound(x) {
  const n = Number(x);
  if (!Number.isFinite(n)) return n;
  return Math.sign(n) * Math.round(Math.abs(n));
}";

/// Нативный `Math.hypot` защищён от переполнения, а `Матан.гипотенуза` интерпретатора
/// считает наивный `sqrt(Σxᵢ²)` и потому даёт `Infinity` уже на `1e200, 1e200`.
/// Шим повторяет наивную формулу и порядок суммирования интерпретатора.
const HYPOT_SRC: &str = r"function __ypsHypot(...args) {
  let sum = 0;
  for (const a of args) {
    const n = Number(a);
    sum += n * n;
  }
  return Math.sqrt(sum);
}";

pub(crate) const STDIN_SRC: &str = r#"let __ypsStdinText = null;
let __ypsStdinPos = 0;
function __ypsStdin() {
  if (__ypsStdinText === null) {
    try {
      __ypsStdinText = require("node:fs").readFileSync(0, "utf8");
    } catch {
      __ypsStdinText = "";
    }
  }
  return __ypsStdinText;
}"#;

const READ_LINE_SRC: &str = r#"function __ypsReadLine() {
  const text = __ypsStdin();
  if (__ypsStdinPos >= text.length) return null;
  const nl = text.indexOf("\n", __ypsStdinPos);
  if (nl === -1) {
    const rest = text.slice(__ypsStdinPos);
    __ypsStdinPos = text.length;
    return rest;
  }
  let line = text.slice(__ypsStdinPos, nl);
  if (line.endsWith("\r")) line = line.slice(0, -1);
  __ypsStdinPos = nl + 1;
  return line;
}"#;

const READ_ALL_SRC: &str = r"function __ypsReadAll() {
  const text = __ypsStdin();
  const rest = text.slice(__ypsStdinPos);
  __ypsStdinPos = text.length;
  return rest;
}";

#[derive(Debug, Clone, Copy)]
pub(crate) enum Builtin {
    Plain(&'static str),
    Helper(Helper),
    Construct(&'static str),
    Length,
    IsError,
}

pub(crate) fn lookup(name: &str) -> Option<Builtin> {
    let mapped = match name {
        "сказать" => Builtin::Plain("console.log"),
        "длина" => Builtin::Length,
        "тип" => Builtin::Helper(Helper::Typeof),
        "число" => Builtin::Plain("Number"),
        "БигЦелое" => Builtin::Plain("BigInt"),
        "строка" => Builtin::Plain("String"),
        "втолкнуть" => Builtin::Helper(Helper::Push),
        "этоКосяк" => Builtin::IsError,
        "RegExp" => Builtin::Plain("RegExp"),
        "Дата" => Builtin::Construct("Date"),
        "Косяк" => Builtin::Construct("Error"),
        "чутка" => Builtin::Plain("setTimeout"),
        "отменаЧутки" => Builtin::Plain("clearTimeout"),
        "интервал" => Builtin::Plain("setInterval"),
        "отменаИнтервала" => Builtin::Plain("clearInterval"),
        "сразу" => Builtin::Plain("setImmediate"),
        "наСледующемТике" => Builtin::Plain("process.nextTick"),
        "подождать" => Builtin::Helper(Helper::Sleep),
        "сОчередить" => Builtin::Plain("queueMicrotask"),
        "прочестьСтроку" => Builtin::Helper(Helper::ReadLine),
        "прочестьВсё" => Builtin::Helper(Helper::ReadAll),
        _ => {
            let (namespace, member) = name.split_once('.')?;
            return namespace_member(namespace, member);
        }
    };
    Some(mapped)
}

pub(crate) fn namespace_member(namespace: &str, property: &str) -> Option<Builtin> {
    let js = match (namespace, property) {
        ("Матан", "округлить") => return Some(Builtin::Helper(Helper::Round)),
        ("Матан", "гипотенуза") => return Some(Builtin::Helper(Helper::Hypot)),
        ("сказать", "ошибка") => "console.error",
        ("сказать", "предупреждение") => "console.warn",
        ("сказать", "инфо") => "console.info",
        ("сказать", "отладка") => "console.debug",
        ("сказать", "таблица") => "console.table",
        ("сказать", "время") => "console.time",
        ("сказать", "времяСтоп") => "console.timeEnd",
        ("Матан", "ПИ") => "Math.PI",
        ("Матан", "Е") => "Math.E",
        ("Матан", "ЛН2") => "Math.LN2",
        ("Матан", "ЛН10") => "Math.LN10",
        ("Матан", "ЛОГ2Е") => "Math.LOG2E",
        ("Матан", "ЛОГ10Е") => "Math.LOG10E",
        ("Матан", "КОРЕНЬ2") => "Math.SQRT2",
        ("Матан", "КОРЕНЬ0_5") => "Math.SQRT1_2",
        ("Матан", "пол") => "Math.floor",
        ("Матан", "потолок") => "Math.ceil",
        ("Матан", "модуль") => "Math.abs",
        ("Матан", "мин") => "Math.min",
        ("Матан", "макс") => "Math.max",
        ("Матан", "степень") => "Math.pow",
        ("Матан", "корень") => "Math.sqrt",
        ("Матан", "рандом") => "Math.random",
        ("Матан", "знак") => "Math.sign",
        ("Матан", "обрезать") => "Math.trunc",
        ("Матан", "лог") => "Math.log",
        ("Матан", "синус") => "Math.sin",
        ("Матан", "косинус") => "Math.cos",
        ("Матан", "тангенс") => "Math.tan",
        ("Матан", "арксинус") => "Math.asin",
        ("Матан", "арккосинус") => "Math.acos",
        ("Матан", "арктангенс") => "Math.atan",
        ("Матан", "арктангенс2") => "Math.atan2",
        ("Матан", "кубическийКорень") => "Math.cbrt",
        ("Матан", "лог2") => "Math.log2",
        ("Матан", "лог10") => "Math.log10",
        ("Матан", "лог1п") => "Math.log1p",
        ("Матан", "эксп") => "Math.exp",
        ("Матан", "экспМ1") => "Math.expm1",
        ("Матан", "гиперСинус") => "Math.sinh",
        ("Матан", "гиперКосинус") => "Math.cosh",
        ("Матан", "гиперТангенс") => "Math.tanh",
        ("Матан", "аркГиперСинус") => "Math.asinh",
        ("Матан", "аркГиперКосинус") => "Math.acosh",
        ("Матан", "аркГиперТангенс") => "Math.atanh",
        ("Матан", "дробь32") => "Math.fround",
        ("Матан", "нулиСлева32") => "Math.clz32",
        ("Матан", "умножить32") => "Math.imul",
        ("Жсон", "разобрать") => "JSON.parse",
        ("Жсон", "вСтроку") => "JSON.stringify",
        ("Отражение", "получить") => "Reflect.get",
        ("Отражение", "установить") => "Reflect.set",
        ("Отражение", "есть") => "Reflect.has",
        ("Отражение", "удалить") => "Reflect.deleteProperty",
        ("Отражение", "прототипОт") => "Reflect.getPrototypeOf",
        ("Отражение", "назначитьПрототип") => "Reflect.setPrototypeOf",
        ("Отражение", "собственныеКлючи") => "Reflect.ownKeys",
        ("Отражение", "определитьСвойство") => "Reflect.defineProperty",
        ("Отражение", "описатьСвойство") => "Reflect.getOwnPropertyDescriptor",
        ("Отражение", "расширяем") => "Reflect.isExtensible",
        ("Отражение", "запретитьРасширение") => "Reflect.preventExtensions",
        ("Отражение", "применить") => "Reflect.apply",
        ("Отражение", "построить") => "Reflect.construct",
        _ => return None,
    };
    Some(Builtin::Plain(js))
}

const SUPPORTED_NAMESPACES: &[&str] = &["Матан", "Жсон", "Отражение"];

pub(crate) fn is_supported_namespace(name: &str) -> bool {
    SUPPORTED_NAMESPACES.contains(&name)
}

const UNSUPPORTED_GLOBALS: &[&str] = &[
    "Матан",
    "Кент",
    "Помойка",
    "Хуйня",
    "Жсон",
    "Итератор",
    "Отражение",
    "ФС",
    "Процесс",
    "Сеть",
    "Строка",
    "Карта",
    "Набор",
    "СлабаяКарта",
    "СлабыйНабор",
    "СлабаяСсылка",
    "РеестрФинализации",
    "Симбол",
    "Посредник",
    "СловоПацана",
    "КонтроллёрОтмены",
    "СигналОтмены",
    "Ц8Массив",
    "Ц8ОграниченныйМассив",
    "Ч8Массив",
    "Ц16Массив",
    "Ч16Массив",
    "Ц32Массив",
    "Ч32Массив",
    "Др32Массив",
    "Др64Массив",
    "ОбластьБайтов",
    "ОбзорБайтов",
];

pub(crate) fn is_unsupported_global(name: &str) -> bool {
    UNSUPPORTED_GLOBALS.contains(&name)
}
