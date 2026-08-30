use crate::definition::{DeclKind, Declaration};

#[must_use]
pub fn declaration_hover(decl: &Declaration) -> String {
    match &decl.kind {
        DeclKind::Function { params } => format!("**{}**({}) — функция", decl.name, params.join(", ")),
        DeclKind::Var => format!("**{}**: переменная", decl.name),
        DeclKind::Const => format!("**{}**: константа", decl.name),
        DeclKind::Param => format!("**{}**: параметр", decl.name),
        DeclKind::CatchParam => format!("**{}**: параметр catch", decl.name),
        DeclKind::Class => format!("**{}**: класс", decl.name),
        DeclKind::Import => format!("**{}**: импорт", decl.name),
    }
}

#[must_use]
pub fn keyword_hover(word: &str) -> Option<&'static str> {
    match word {
        "йопта" => Some("**function** — объявление функции"),
        "гыы" => Some("**var** — объявление переменной"),
        "ясенХуй" | "ЯсенХуй" => Some("**const** — объявление константы"),
        "участковый" => Some("**let** — объявление переменной"),
        "вилкойвглаз" => Some("**if** — условный оператор"),
        "иливжопураз" => Some("**else** — ветвь else"),
        "потрещим" => Some("**while** — цикл while"),
        "го" => Some("**for** — цикл for"),
        "харэ" => Some("**break** — прервать цикл"),
        "двигай" => Some("**continue** — следующая итерация"),
        "отвечаю" => Some("**return** — вернуть значение"),
        "правда" | "трулио" | "чётко" | "четко" | "чотко" => {
            Some("**true** — булево истина")
        }
        "лож" | "нетрулио" | "пиздишь" | "нечётко" | "нечетко" | "нечотко" => {
            Some("**false** — булево ложь")
        }
        "ноль" | "нуллио" | "порожняк" => Some("**null** — нулевое значение"),
        "неибу" => Some("**undefined** — неопределённое значение"),
        "хапнуть" | "побратски" | "пабрацки" | "пабратски" => {
            Some("**try** — блок try")
        }
        "гоп" | "аченетак" | "аченитак" | "ачёнетак" => Some("**catch** — поймать ошибку"),
        "тюряжка" => Some("**finally** — блок finally"),
        "кидай" | "пнх" => Some("**throw** — бросить ошибку"),
        "клёво" | "клево" => Some("**class** — объявление класса"),
        "батя" => Some("**extends** — наследование"),
        "яга" => Some("**super** — обращение к родителю"),
        "захуярить" | "гыйбать" => Some("**new** — создать экземпляр"),
        "тырыпыры" => Some("**this** — текущий объект"),
        "попонятия" => Some("**static** — статический член"),
        "чезажижан" => Some("**typeof** — тип значения"),
        "шкура" => Some("**instanceof** — проверка типа"),
        "пиздюли" => Some("**function\\*** — функция-генератор"),
        "поебалу" => Some("**yield** — отдать значение из генератора"),
        "поебалуна" => Some("**yield\\*** — делегировать генератор"),
        "ассо" => Some("**async** — асинхронная функция"),
        "сидетьНахуй" => Some("**await** — ожидать промис"),
        "спиздить" => Some("**import** — импорт модуля"),
        "предъява" => Some("**export** — экспорт"),
        "откуда" => Some("**from** — источник импорта"),
        "сашаГрей" => Some("**of** — итерация (for-of)"),
        "из" | "чоунастут" => Some("**in** — итерация (for-in) / оператор in"),
        "ёбнуть" | "ебнуть" => Some("**delete** — удалить свойство"),
        "куку" => Some("**void** — вычислить и вернуть undefined"),
        "юзай" => Some("**using** — управление ресурсом"),
        "базарпо" | "естьчо" => Some("**switch** — множественный выбор"),
        "тема" | "лещ" | "аеслинайду" => Some("**case** — ветвь switch"),
        "нуичо" | "пахану" | "апохуй" | "наотыбись" => {
            Some("**default** — ветвь по умолчанию")
        }
        "крутани" | "крч" => Some("**do** — цикл do-while"),
        "логопед" => Some("**debugger** — точка останова отладчика"),
        "мой" => Some("**private** — приватный член класса"),
        "подкрыша" => Some("**protected** — защищённый член класса"),
        "ебанное" => Some("**public** — публичный член класса"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_keyword_has_doc() {
        assert!(keyword_hover("йопта").unwrap().contains("function"));
    }

    #[test]
    fn unknown_word_has_no_doc() {
        assert!(keyword_hover("foobar").is_none());
    }

    #[test]
    fn every_keyword_has_a_doc() {
        let missing: Vec<&str> = yps_lexer::KEYWORDS.iter().copied().filter(|k| keyword_hover(k).is_none()).collect();
        assert!(missing.is_empty(), "keywords without hover docs: {missing:?}");
    }

    fn decl_hover_at(src: &str, needle: &str) -> Option<String> {
        let analyzed = crate::analyze(src);
        let byte = src.rfind(needle).unwrap();
        let word = crate::position::word_at(src, byte);
        analyzed.declarations.iter().find(|d| d.name == word).map(declaration_hover)
    }

    #[test]
    fn hovers_user_function_with_params() {
        let src = "йопта фу(парам1, парам2) { отвечаю парам1; }\nфу(1, 2);";
        let doc = decl_hover_at(src, "фу").unwrap();
        assert!(doc.contains("фу"));
        assert!(doc.contains("парам1, парам2"));
        assert!(doc.contains("функция"));
    }

    #[test]
    fn hovers_const_declaration() {
        let src = "ясенХуй x = 1;\nсказать(x);";
        let doc = decl_hover_at(src, "x").unwrap();
        assert!(doc.contains("константа"));
    }

    #[test]
    fn hovers_var_style_declaration() {
        let var_doc = decl_hover_at("гыы x = 1;\nсказать(x);", "x").unwrap();
        let let_doc = decl_hover_at("участковый y = 1;\nсказать(y);", "y").unwrap();
        assert!(var_doc.contains("переменная"));
        assert!(let_doc.contains("переменная"));
    }

    #[test]
    fn hovers_function_parameter() {
        let src = "йопта фу(парам) { отвечаю парам; }";
        let doc = decl_hover_at(src, "парам").unwrap();
        assert!(doc.contains("параметр"));
    }

    #[test]
    fn no_hover_for_undeclared_identifier() {
        let src = "неизвестно;";
        let analyzed = crate::analyze(src);
        let word = crate::position::word_at(src, 0);
        assert!(analyzed.declarations.iter().find(|d| d.name == word).is_none());
    }
}
