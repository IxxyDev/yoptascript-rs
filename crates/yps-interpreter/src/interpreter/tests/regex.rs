use super::*;

#[test]
fn regex_literal_test_and_find() {
    let interp = run_code(
        r#"
        гыы шаблон = /\d+/;
        гыы есть = шаблон.проверить("номер 42");
        гыы найдено = шаблон.найти("abc 123 def");
        гыы первое = найдено["0"];
        гыы idx = найдено.index;
        "#,
    );
    assert_eq!(interp.get("есть"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("первое"), Some(Value::String("123".to_string())));
    assert_eq!(interp.get("idx"), Some(Value::Number(4.0)));
}

#[test]
fn regex_str_match_no_g() {
    let interp = run_code(
        r#"
        гыы r = "abc 123 def".совпадает(/\d+/);
        гыы m = r["0"];
        "#,
    );
    assert_eq!(interp.get("m"), Some(Value::String("123".to_string())));
}

#[test]
fn regex_str_match_no_match() {
    let interp = run_code(
        r#"
        гыы r = "abc".совпадает(/\d+/);
        "#,
    );
    assert_eq!(interp.get("r"), Some(Value::Null));
}

#[test]
fn regex_str_match_global() {
    let interp = run_code(
        r#"
        гыы r = "a1 b2 c3".совпадает(/\d/g);
        гыы a = r[0];
        гыы b = r[1];
        гыы c = r[2];
        "#,
    );
    assert_eq!(interp.get("a"), Some(Value::String("1".to_string())));
    assert_eq!(interp.get("b"), Some(Value::String("2".to_string())));
    assert_eq!(interp.get("c"), Some(Value::String("3".to_string())));
}

#[test]
fn regex_str_match_all() {
    let interp = run_code(
        r#"
        гыы first = "";
        гыы second_g1 = "";
        гыы i = 0;
        го (м сашаГрей "a1 b2".найтиВсе(/(\w)(\d)/g)) {
            вилкойвглаз (i == 0) { first = м["0"]; }
            вилкойвглаз (i == 1) { second_g1 = м["1"]; }
            i = i + 1;
        }
        "#,
    );
    assert_eq!(interp.get("first"), Some(Value::String("a1".to_string())));
    assert_eq!(interp.get("second_g1"), Some(Value::String("b".to_string())));
}

#[test]
fn regex_matchall_lazy_iterator() {
    let interp = run_code(
        r#"
        гыы out = "";
        го (м сашаГрей "a1 b2 c3".найтиВсе(/\d/g)) {
            out = out + м["0"];
        }
        "#,
    );
    assert_eq!(interp.get("out"), Some(Value::String("123".to_string())));
}

#[test]
fn regex_matchall_returns_iterator_type() {
    let interp = run_code(
        r#"
        гыы t = тип("x".найтиВсе(/x/g));
        "#,
    );
    assert_eq!(interp.get("t"), Some(Value::String("итератор".to_string())));
}

#[test]
fn regex_str_replace() {
    let interp = run_code(
        r#"
        гыы r = "hello world".заменить(/world/, "yopta");
        "#,
    );
    assert_eq!(interp.get("r"), Some(Value::String("hello yopta".to_string())));
}

#[test]
fn regex_str_replace_global() {
    let interp = run_code(
        r#"
        гыы r = "a-b-c".заменить(/-/g, "_");
        "#,
    );
    assert_eq!(interp.get("r"), Some(Value::String("a_b_c".to_string())));
}

#[test]
fn regex_str_replace_backref() {
    let interp = run_code(
        r#"
        гыы r = "John Smith".заменить(/(\w+) (\w+)/, "$2 $1");
        "#,
    );
    assert_eq!(interp.get("r"), Some(Value::String("Smith John".to_string())));
}

#[test]
fn regex_str_replace_dollar_escape() {
    let interp = run_code(
        r#"
        гыы r = "abc".заменить(/b/, "$$");
        "#,
    );
    assert_eq!(interp.get("r"), Some(Value::String("a$c".to_string())));
}

#[test]
fn regex_str_replace_named_backref() {
    let interp = run_code(
        r#"
        гыы r = "John Smith".заменить(/(?<first>\w+) (?<last>\w+)/, "$<last> $<first>");
        "#,
    );
    assert_eq!(interp.get("r"), Some(Value::String("Smith John".to_string())));
}

#[test]
fn regex_replace_with_fn() {
    let interp = run_code(
        r#"
        гыы r = "a1b2".заменить(/\d/g, (m) => число(m) * 10 + "");
        "#,
    );
    assert_eq!(interp.get("r"), Some(Value::String("a10b20".to_string())));
}

#[test]
fn regex_replace_with_fn_groups() {
    let interp = run_code(
        r#"
        гыы r = "foo bar".заменить(/(\w+) (\w+)/, (m, a, b) => b + " " + a);
        "#,
    );
    assert_eq!(interp.get("r"), Some(Value::String("bar foo".to_string())));
}

#[test]
fn regex_replace_with_fn_offset() {
    let interp = run_code(
        r#"
        гыы r = "abc".заменить(/./g, (m, off) => off + "");
        "#,
    );
    assert_eq!(interp.get("r"), Some(Value::String("012".to_string())));
}

#[test]
fn regex_replace_with_fn_no_g_only_first() {
    let interp = run_code(
        r#"
        гыы r = "a1b2c3".заменить(/\d/, (m) => "X");
        "#,
    );
    assert_eq!(interp.get("r"), Some(Value::String("aXb2c3".to_string())));
}

#[test]
fn regex_replace_all_with_fn() {
    let interp = run_code(
        r#"
        гыы r = "a1b2".заменитьВсе(/\d/g, (m) => число(m) + 1 + "");
        "#,
    );
    assert_eq!(interp.get("r"), Some(Value::String("a2b3".to_string())));
}

#[test]
fn regex_str_replace_multi_digit_backref() {
    let interp = run_code(
        r#"
        гыы a = "abc".заменить(/(a)(b)(c)/, "$3$2$1");
        гыы b = "XabcdefghijY".заменить(/(a)(b)(c)(d)(e)(f)(g)(h)(i)(j)/, "$10$9$8");
        гыы c = "aZ".заменить(/(a)/, "$10");
        "#,
    );
    assert_eq!(interp.get("a"), Some(Value::String("cba".to_string())));
    assert_eq!(interp.get("b"), Some(Value::String("XjihY".to_string())));
    assert_eq!(interp.get("c"), Some(Value::String("a0Z".to_string())));
}

#[test]
fn regex_str_split() {
    let interp = run_code(
        r#"
        гыы r = "a, b,  c,d".разбить(/,\s*/);
        гыы a = r[0];
        гыы b = r[1];
        гыы c = r[2];
        гыы d = r[3];
        "#,
    );
    assert_eq!(interp.get("a"), Some(Value::String("a".to_string())));
    assert_eq!(interp.get("b"), Some(Value::String("b".to_string())));
    assert_eq!(interp.get("c"), Some(Value::String("c".to_string())));
    assert_eq!(interp.get("d"), Some(Value::String("d".to_string())));
}

#[test]
fn regex_str_search() {
    let interp = run_code(
        r#"
        гыы a = "abc 123".найтиИндекс(/\d+/);
        гыы b = "abc".найтиИндекс(/\d+/);
        "#,
    );
    assert_eq!(interp.get("a"), Some(Value::Number(4.0)));
    assert_eq!(interp.get("b"), Some(Value::Number(-1.0)));
}

#[test]
fn regex_exec_object_shape() {
    let interp = run_code(
        r#"
        гыы r = /(\w+)/.найти("hello");
        гыы whole = r["0"];
        гыы g1 = r["1"];
        гыы idx = r.index;
        гыы inp = r.input;
        гыы grp = r.groups;
        "#,
    );
    assert_eq!(interp.get("whole"), Some(Value::String("hello".to_string())));
    assert_eq!(interp.get("g1"), Some(Value::String("hello".to_string())));
    assert_eq!(interp.get("idx"), Some(Value::Number(0.0)));
    assert_eq!(interp.get("inp"), Some(Value::String("hello".to_string())));
    assert_eq!(interp.get("grp"), Some(Value::Null));
}

#[test]
fn regex_exec_named_groups() {
    let interp = run_code(
        r#"
        гыы r = /(?<word>\w+)/.найти("hi");
        гыы w = r.groups.word;
        "#,
    );
    assert_eq!(interp.get("w"), Some(Value::String("hi".to_string())));
}

#[test]
fn regex_lastindex_global_advances() {
    let interp = run_code(
        r#"
        гыы re = /\d/g;
        гыы r1 = re.найти("a1b2");
        гыы li1 = re.последнийИндекс;
        гыы r2 = re.найти("a1b2");
        гыы li2 = re.последнийИндекс;
        гыы r3 = re.найти("a1b2");
        гыы li3 = re.последнийИндекс;
        "#,
    );
    assert_eq!(interp.get("li1"), Some(Value::Number(2.0)));
    assert_eq!(interp.get("li2"), Some(Value::Number(4.0)));
    assert_eq!(interp.get("li3"), Some(Value::Number(0.0)));
    assert_eq!(interp.get("r3"), Some(Value::Null));
}

#[test]
fn regex_new_regexp_with_flags() {
    let interp = run_code(
        r#"
        гыы re = RegExp("hello", "i");
        гыы ok = re.проверить("HELLO");
        "#,
    );
    assert_eq!(interp.get("ok"), Some(Value::Boolean(true)));
}

#[test]
fn regex_new_regexp_no_flags() {
    let interp = run_code(
        r#"
        гыы re = RegExp("abc");
        гыы ok = re.проверить("xabcx");
        гыы fl = re.флаги;
        "#,
    );
    assert_eq!(interp.get("ok"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("fl"), Some(Value::String(String::new())));
}

#[test]
fn regex_new_regexp_from_regex_with_flags() {
    let interp = run_code(
        r#"
        гыы base = /hello/;
        гыы re = RegExp(base, "i");
        гыы ok = re.проверить("HELLO");
        гыы fl = re.флаги;
        "#,
    );
    assert_eq!(interp.get("ok"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("fl"), Some(Value::String("i".to_string())));
}

#[test]
fn regex_new_regexp_invalid_pattern() {
    let err = run_code_err(
        r#"
        RegExp("(unclosed");
        "#,
    );
    assert!(err.message.contains("(unclosed"), "got: {}", err.message);
}

#[test]
fn regex_lookbehind_rejected() {
    let err = run_code_err(
        r#"
        RegExp("(?<=foo)bar");
        "#,
    );
    assert!(err.message.contains("lookbehind"), "got: {}", err.message);
}

#[test]
fn regex_backref_rejected() {
    let err = run_code_err(
        r#"
        RegExp("(a)\\1");
        "#,
    );
    assert!(err.message.contains("backreferences"), "got: {}", err.message);
}

#[test]
fn regex_sticky_match_at_position() {
    let interp = run_code(
        r#"
        гыы re = RegExp("\\d", "y");
        re.последнийИндекс = 1;
        гыы r = re.найти("a1b2");
        гыы matched = r["0"];
        гыы li = re.последнийИндекс;
        "#,
    );
    assert_eq!(interp.get("matched"), Some(Value::String("1".to_string())));
    assert_eq!(interp.get("li"), Some(Value::Number(2.0)));
}

#[test]
fn regex_sticky_mismatch_resets() {
    let interp = run_code(
        r#"
        гыы re = RegExp("\\d", "y");
        гыы r = re.найти("a1b2");
        гыы li = re.последнийИндекс;
        "#,
    );
    assert_eq!(interp.get("r"), Some(Value::Null));
    assert_eq!(interp.get("li"), Some(Value::Number(0.0)));
}

#[test]
fn regex_indices_in_exec() {
    let interp = run_code(
        r#"
        гыы re = RegExp("(\\d)(\\w)", "d");
        гыы r = re.найти("a1b");
        гыы pair0 = r.indices["0"];
        гыы pair1 = r.indices["1"];
        гыы pair2 = r.indices["2"];
        гыы s0 = pair0[0];
        гыы e0 = pair0[1];
        гыы s1 = pair1[0];
        гыы e1 = pair1[1];
        гыы s2 = pair2[0];
        гыы e2 = pair2[1];
        "#,
    );
    assert_eq!(interp.get("s0"), Some(Value::Number(1.0)));
    assert_eq!(interp.get("e0"), Some(Value::Number(3.0)));
    assert_eq!(interp.get("s1"), Some(Value::Number(1.0)));
    assert_eq!(interp.get("e1"), Some(Value::Number(2.0)));
    assert_eq!(interp.get("s2"), Some(Value::Number(2.0)));
    assert_eq!(interp.get("e2"), Some(Value::Number(3.0)));
}

#[test]
fn regex_indices_groups() {
    let interp = run_code(
        r#"
        гыы re = RegExp("(?<n>\\d+)", "d");
        гыы r = re.найти("a42b");
        гыы pair = r.indices.groups.n;
        гыы s = pair[0];
        гыы e = pair[1];
        "#,
    );
    assert_eq!(interp.get("s"), Some(Value::Number(1.0)));
    assert_eq!(interp.get("e"), Some(Value::Number(3.0)));
}

#[test]
fn regex_lastindex_property_read() {
    let interp = run_code(
        r#"
        гыы re = /\d+/g;
        re.найти("abc 123 def");
        гыы li = re.последнийИндекс;
        "#,
    );
    assert_eq!(interp.get("li"), Some(Value::Number(7.0)));
}

#[test]
fn regex_sticky_property_flag() {
    let interp = run_code(
        r#"
        гыы re = RegExp("a", "y");
        гыы s = re.sticky;
        гыы s2 = re.липкий;
        "#,
    );
    assert_eq!(interp.get("s"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("s2"), Some(Value::Boolean(true)));
}

#[test]
fn regex_hasindices_property_flag() {
    let interp = run_code(
        r#"
        гыы re = RegExp("a", "d");
        гыы h = re.hasIndices;
        гыы h2 = re.имеетИндексы;
        "#,
    );
    assert_eq!(interp.get("h"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("h2"), Some(Value::Boolean(true)));
}

#[test]
fn regex_lastindex_property_write() {
    let interp = run_code(
        r#"
        гыы re = /\d/g;
        re.последнийИндекс = 2;
        гыы r = re.найти("a1b2");
        гыы matched = r["0"];
        "#,
    );
    assert_eq!(interp.get("matched"), Some(Value::String("2".to_string())));
}

#[test]
fn regex_escape_safe_literal_paren() {
    let interp = run_code(
        r#"
        гыы re = RegExp("\\(\\?<x>foo\\)");
        гыы ok = re.проверить("(?<x>foo)");
        "#,
    );
    assert_eq!(interp.get("ok"), Some(Value::Boolean(true)));
}

#[test]
fn regex_escape_safe_char_class() {
    let interp = run_code(
        r#"
        гыы re = RegExp("[(?<x>]");
        гыы a = re.проверить("(");
        гыы b = re.проверить("?");
        гыы c = re.проверить("<");
        гыы d = re.проверить("x");
        гыы e = re.проверить(">");
        "#,
    );
    assert_eq!(interp.get("a"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("b"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("c"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("d"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("e"), Some(Value::Boolean(true)));
}

#[test]
fn regex_case_insensitive_flag() {
    let interp = run_code(
        r#"
        гыы p = /hello/i;
        гыы r = p.проверить("Hello World");
        "#,
    );
    assert_eq!(interp.get("r"), Some(Value::Boolean(true)));
}

#[test]
fn regex_source_and_flags_properties() {
    let interp = run_code(
        r#"
        гыы p = /foo/gi;
        гыы src = p.источник;
        гыы fl = p.флаги;
        "#,
    );
    assert_eq!(interp.get("src"), Some(Value::String("foo".to_string())));
    assert_eq!(interp.get("fl"), Some(Value::String("gi".to_string())));
}

#[test]
fn regex_division_disambiguation() {
    let interp = run_code(
        r#"
        гыы a = 10;
        гыы b = 2;
        гыы c = a / b / 1;
        "#,
    );
    assert_eq!(interp.get("c"), Some(Value::Number(5.0)));
}
