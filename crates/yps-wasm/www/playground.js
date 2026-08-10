import init, { run_yopta } from "./pkg/yps_wasm.js";

const EXAMPLES = [
  { name: "hello", title: { ru: "Основы: переменные, условия, функции", en: "Basics: variables, conditionals, functions" }, source: "// Булевы литералы\nгыы флаг = правда;\nсказать(\"флаг:\", флаг);\nсказать(\"!флаг:\", !флаг);\nсказать(\"ноль:\", ноль);\n\nвилкойвглаз (флаг) {\n    сказать(\"флаг - правда\");\n}\n\nвилкойвглаз (лож) {\n    сказать(\"это не выведется\");\n} иливжопураз {\n    сказать(\"лож - это лож\");\n}\n\n// Константы\nясенХуй ПИ = 3.14;\nсказать(\"ПИ:\", ПИ);\n\n// Составные операторы\nгыы х = 10;\nх += 5;\nсказать(\"х += 5:\", х);\nх -= 3;\nсказать(\"х -= 3:\", х);\nх *= 2;\nсказать(\"х *= 2:\", х);\nх /= 4;\nсказать(\"х /= 4:\", х);\n\n// Инкремент/декремент\nгыы и = 0;\nпотрещим (и < 5) {\n    сказать(\"и =\", и);\n    и++;\n}\n\nгыы к = 3;\nк--;\nсказать(\"к после --:\", к);\n\n// For с инкрементом\nго (гыы н = 0; н < 3; н++) {\n    сказать(\"н =\", н);\n}\n\n// Рекурсия\nйопта факториал(н) {\n    вилкойвглаз (н <= 1) {\n        отвечаю 1;\n    }\n    отвечаю н * факториал(н - 1);\n}\nсказать(\"факториал(5) =\", факториал(5));\n" },
  { name: "labeled_loops", title: { ru: "Помеченные циклы: харэ/двигай по метке", en: "Labeled loops: харэ/двигай with a label" }, source: "// Помеченные циклы: харэ/двигай по метке\n\n// harэ по метке прерывает ВНЕШНИЙ цикл\nпоиск: го (гыы и = 0; и < 3; и += 1) {\n    го (гыы ж = 0; ж < 3; ж += 1) {\n        вилкойвглаз (и == 1 && ж == 1) {\n            сказать(\"нашли на \" + и + \",\" + ж);\n            харэ поиск;\n        }\n    }\n}\n\n// двигай по метке продолжает ВНЕШНИЙ цикл, пропуская остаток внутреннего\nстроки: го (гыы р = 0; р < 3; р += 1) {\n    го (гыы к = 0; к < 3; к += 1) {\n        вилкойвглаз (к == 1) {\n            двигай строки;\n        }\n        сказать(\"ячейка \" + р + \",\" + к);\n    }\n}\n" },
  { name: "iterator_helpers", title: { ru: "Ленивые итераторы и их методы", en: "Lazy iterators and their helpers" }, source: "гыы числа = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];\n\nгыы квадратыЧётных = Итератор.от(числа)\n    .отфильтровать((н) => н % 2 == 0)\n    .преобразовать((н) => н * н)\n    .вМассив();\nсказать(квадратыЧётных);\n\nгыы первыеТри = Итератор.от(числа).взять(3).вМассив();\nсказать(первыеТри);\n\nгыы безПервыхДвух = Итератор.от(числа).пропустить(2).взять(3).вМассив();\nсказать(безПервыхДвух);\n\nгыы сумма = Итератор.от(числа).свернуть((а, б) => а + б, 0);\nсказать(сумма);\n\nгыы все = Итератор.склеить([1, 2], [3, 4], [5, 6]).вМассив();\nсказать(все);\n\nго (х сашаГрей Итератор.от([10, 20, 30]).преобразовать((н) => н + 1)) {\n    сказать(х);\n}\n\nгыы и = Итератор.от([\"а\", \"б\", \"в\"]);\nсказать(и.следующий());\nсказать(и.следующий());\nсказать(и.следующий());\nсказать(и.следующий());\n" },
  { name: "stdlib", title: { ru: "Стандартная библиотека: Матан, Помойка, Строки", en: "Standard library: Матан, Помойка, strings" }, source: "сказать(\"=== Матан ===\");\nсказать(\"ПИ =\", Матан.ПИ);\nсказать(\"Е =\", Матан.Е);\nсказать(\"пол(3.7) =\", Матан.пол(3.7));\nсказать(\"потолок(3.2) =\", Матан.потолок(3.2));\nсказать(\"мин(5, 2, 8) =\", Матан.мин(5, 2, 8));\nсказать(\"макс(5, 2, 8) =\", Матан.макс(5, 2, 8));\nсказать(\"степень(2, 10) =\", Матан.степень(2, 10));\nсказать(\"корень(144) =\", Матан.корень(144));\n\nсказать(\"\");\nсказать(\"=== Помойка (методы массива) ===\");\nгыы числа = [3, 1, 4, 1, 5, 9, 2, 6];\nсказать(\"длина:\", числа.length);\nсказать(\"повёрнуто:\", числа.toReversed());\nсказать(\"отсортировано:\", числа.toSorted((а, б) => а - б));\n\nгыы квадраты = [1, 2, 3, 4, 5].map((x) => x * x);\nсказать(\"квадраты:\", квадраты);\n\nгыы чётные = [1, 2, 3, 4, 5, 6].filter((x) => x % 2 === 0);\nсказать(\"чётные:\", чётные);\n\nгыы сумма = [1, 2, 3, 4, 5].reduce((а, б) => а + б, 0);\nсказать(\"сумма 1..5 =\", сумма);\n\nгыы а = [1, 2, 3];\nа.push(4);\nа.push(5);\nсказать(\"после push:\", а);\nсказать(\"последний (at -1):\", а.at(-1));\n\nсказать(\"\");\nсказать(\"=== Строка ===\");\nгыы с = \"Привет, Мир!\";\nсказать(\"длина:\", с.length);\nсказать(\"в верхнем:\", с.toUpperCase());\nсказать(\"в нижнем:\", с.toLowerCase());\nсказать(\"разбито по запятой:\", с.split(\", \"));\nсказать(\"включает Мир:\", с.includes(\"Мир\"));\nсказать(\"замена:\", с.replace(\"Мир\", \"Брат\"));\n\nгыы число = \"5\".padStart(3, \"0\");\nсказать(\"дополнено:\", число);\n\nсказать(\"\");\nсказать(\"=== Кент (Object) ===\");\nгыы пацан = { имя: \"Саня\", возраст: 30, район: \"Юг\" };\nсказать(\"ключи:\", Кент.ключи(пацан));\nсказать(\"значения:\", Кент.значения(пацан));\nсказать(\"записи:\", Кент.записи(пацан));\n\nсказать(\"\");\nсказать(\"=== Жсон ===\");\nгыы json = Жсон.вСтроку(пацан);\nсказать(\"сериализовано:\", json);\nгыы распарсено = Жсон.разобрать(json);\nсказать(\"имя после разбора:\", распарсено.имя);\n\nсказать(\"\");\nсказать(\"=== Хуйня (Number) ===\");\nсказать(\"конечна(42):\", Хуйня.конечна(42));\nсказать(\"целая(3.14):\", Хуйня.целая(3.14));\nсказать(\"целая(42):\", Хуйня.целая(42));\n\nсказать(\"\");\nсказать(\"=== Цепочки ===\");\nгыы ответ = [1, 2, 3, 4, 5]\n    .filter((x) => x > 1)\n    .map((x) => x * x)\n    .reduce((а, б) => а + б, 0);\nсказать(\"сумма квадратов (2..5) =\", ответ);\n" },
];

const STRINGS = {
  ru: {
    docTitle: "Песочница YoptaScript",
    title: "Песочница YoptaScript",
    subtitle: "Код выполняется в браузере через WebAssembly. Файловая система, сеть и процесс недоступны.",
    examplesLabel: "Пример:",
    run: "Запустить",
    loading: "Загрузка модуля…",
    ready: "Модуль загружен",
    loadFailed: "Не удалось загрузить wasm-модуль",
    empty: "(пусто)",
    outputTitle: "Вывод",
  },
  en: {
    docTitle: "YoptaScript Playground",
    title: "YoptaScript Playground",
    subtitle: "Code runs in your browser via WebAssembly. File system, network and process are unavailable.",
    examplesLabel: "Example:",
    run: "Run",
    loading: "Loading module…",
    ready: "Module loaded",
    loadFailed: "Failed to load the wasm module",
    empty: "(empty)",
    outputTitle: "Output",
  },
};

const sourceEl = document.getElementById("source");
const outputEl = document.getElementById("output");
const runEl = document.getElementById("run");
const statusEl = document.getElementById("status");
const examplesEl = document.getElementById("examples");
const titleEl = document.getElementById("title");
const subtitleEl = document.getElementById("subtitle");
const examplesLabelEl = document.getElementById("examples-label");
const outputTitleEl = document.getElementById("output-title");
const langButtons = {
  ru: document.getElementById("lang-ru"),
  en: document.getElementById("lang-en"),
};

let lang = detectLang();
let statusKey = "loading";

function detectLang() {
  const saved = localStorage.getItem("yps-lang");
  if (saved === "ru" || saved === "en") {
    return saved;
  }
  return (navigator.language || "").toLowerCase().startsWith("ru") ? "ru" : "en";
}

function setStatus(key) {
  statusKey = key;
  statusEl.textContent = STRINGS[lang][key];
}

function applyLang(next) {
  lang = next;
  localStorage.setItem("yps-lang", lang);
  const t = STRINGS[lang];
  document.documentElement.lang = lang;
  document.title = t.docTitle;
  titleEl.textContent = t.title;
  subtitleEl.textContent = t.subtitle;
  examplesLabelEl.textContent = t.examplesLabel;
  runEl.textContent = t.run;
  outputTitleEl.textContent = t.outputTitle;
  setStatus(statusKey);
  for (const option of examplesEl.options) {
    const example = EXAMPLES.find((e) => e.name === option.value);
    if (example) {
      option.textContent = example.title[lang];
    }
  }
  for (const [code, button] of Object.entries(langButtons)) {
    button.classList.toggle("active", code === lang);
  }
}

for (const example of EXAMPLES) {
  const option = document.createElement("option");
  option.value = example.name;
  examplesEl.appendChild(option);
}

function loadExample(name) {
  const example = EXAMPLES.find((e) => e.name === name);
  if (example) {
    sourceEl.value = example.source;
  }
}

function show(text, isError) {
  outputEl.textContent = text;
  outputEl.classList.toggle("error", Boolean(isError));
}

function run() {
  let result;
  try {
    result = run_yopta(sourceEl.value);
  } catch (e) {
    show(String(e?.message ?? e), true);
    return;
  }
  show(result === "" ? STRINGS[lang].empty : result, false);
}

examplesEl.addEventListener("change", () => loadExample(examplesEl.value));
runEl.addEventListener("click", run);
langButtons.ru.addEventListener("click", () => applyLang("ru"));
langButtons.en.addEventListener("click", () => applyLang("en"));

loadExample(EXAMPLES[0].name);
applyLang(lang);

init()
  .then(() => {
    runEl.disabled = false;
    setStatus("ready");
  })
  .catch((e) => {
    setStatus("loadFailed");
    show(String(e), true);
  });
