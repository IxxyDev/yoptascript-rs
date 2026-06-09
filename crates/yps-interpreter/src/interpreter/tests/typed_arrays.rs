use super::*;

#[test]
fn ta_construct_with_length_zeroed() {
    let interp = run_code(
        r#"
        гыы т = захуярить Ц8Массив(3);
        гыы дл = длина(т);
        гыы а = т[0];
        гыы б = т[1];
        гыы в = т[2];
        "#,
    );
    assert_eq!(interp.get("дл"), Some(Value::Number(3.0)));
    assert_eq!(interp.get("а"), Some(Value::Number(0.0)));
    assert_eq!(interp.get("б"), Some(Value::Number(0.0)));
    assert_eq!(interp.get("в"), Some(Value::Number(0.0)));
}

#[test]
fn ta_construct_from_array() {
    let interp = run_code(
        r#"
        гыы т = захуярить Ц8Массив([10, 20, 30]);
        гыы дл = длина(т);
        гыы а = т[0];
        гыы в = т[2];
        "#,
    );
    assert_eq!(interp.get("дл"), Some(Value::Number(3.0)));
    assert_eq!(interp.get("а"), Some(Value::Number(10.0)));
    assert_eq!(interp.get("в"), Some(Value::Number(30.0)));
}

#[test]
fn ta_index_read_write() {
    let interp = run_code(
        r#"
        гыы т = захуярить Ц8Массив(2);
        т[0] = 255;
        т[1] = 42;
        гыы а = т[0];
        гыы б = т[1];
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::Number(255.0)));
    assert_eq!(interp.get("б"), Some(Value::Number(42.0)));
}

#[test]
fn ta_overflow_u8_modulo() {
    let interp = run_code(
        r#"
        гыы т = захуярить Ц8Массив(3);
        т[0] = 256;
        т[1] = 257;
        т[2] = -1;
        гыы а = т[0];
        гыы б = т[1];
        гыы в = т[2];
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::Number(0.0)));
    assert_eq!(interp.get("б"), Some(Value::Number(1.0)));
    assert_eq!(interp.get("в"), Some(Value::Number(255.0)));
}

#[test]
fn ta_overflow_i8_wrap() {
    let interp = run_code(
        r#"
        гыы т = захуярить Ч8Массив(2);
        т[0] = 128;
        т[1] = 255;
        гыы а = т[0];
        гыы б = т[1];
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::Number(-128.0)));
    assert_eq!(interp.get("б"), Some(Value::Number(-1.0)));
}

#[test]
fn ta_oob_read_undefined_write_noop() {
    let interp = run_code(
        r#"
        гыы т = захуярить Ц8Массив(2);
        т[0] = 5;
        гыы вне = т[999];
        т[999] = 7;
        гыы дл = длина(т);
        гыы цел = т[0];
        "#,
    );
    assert_eq!(interp.get("вне"), Some(Value::Undefined));
    assert_eq!(interp.get("дл"), Some(Value::Number(2.0)));
    assert_eq!(interp.get("цел"), Some(Value::Number(5.0)));
}

#[test]
fn ta_shared_buffer_views_see_each_other() {
    let interp = run_code(
        r#"
        гыы об = захуярить ОбластьБайтов(4);
        гыы а = захуярить Ц8Массив(об);
        гыы б = захуярить Ц8Массив(об);
        а[0] = 200;
        гыы видит = б[0];
        "#,
    );
    assert_eq!(interp.get("видит"), Some(Value::Number(200.0)));
}

#[test]
fn ta_byte_order_le() {
    let interp = run_code(
        r#"
        гыы об = захуярить ОбластьБайтов(4);
        гыы байты = захуярить Ц8Массив(об);
        гыы слово = захуярить Ц32Массив(об);
        байты[0] = 1;
        байты[1] = 0;
        байты[2] = 0;
        байты[3] = 0;
        гыы зн = слово[0];
        "#,
    );
    assert_eq!(interp.get("зн"), Some(Value::Number(1.0)));
}

#[test]
fn ta_iterable_for_of() {
    let interp = run_code(
        r#"
        гыы т = захуярить Ц8Массив([3, 4, 5]);
        гыы сумма = 0;
        го (х из т) {
            сумма = сумма + х;
        }
        "#,
    );
    assert_eq!(interp.get("сумма"), Some(Value::Number(12.0)));
}

#[test]
fn ta_iterable_for_of_sashagrey() {
    let interp = run_code(
        r#"
        гыы т = захуярить Ц8Массив([2, 3, 4]);
        гыы сумма = 0;
        го (х сашаГрей т) {
            сумма = сумма + х;
        }
        "#,
    );
    assert_eq!(interp.get("сумма"), Some(Value::Number(9.0)));
}

#[test]
fn ta_spread_into_array() {
    let interp = run_code(
        r#"
        гыы т = захуярить Ц8Массив([7, 8]);
        гыы арр = [...т];
        гыы дл = длина(арр);
        гыы а = арр[0];
        гыы б = арр[1];
        "#,
    );
    assert_eq!(interp.get("дл"), Some(Value::Number(2.0)));
    assert_eq!(interp.get("а"), Some(Value::Number(7.0)));
    assert_eq!(interp.get("б"), Some(Value::Number(8.0)));
}

#[test]
fn ta_properties() {
    let interp = run_code(
        r#"
        гыы об = захуярить ОбластьБайтов(16);
        гыы т = захуярить Ц32Массив(об, 4, 2);
        гыы дл = т.длина;
        гыы дб = т.длинаБайт;
        гыы см = т.смещениеБайт;
        гыы аб = об.длинаБайт;
        "#,
    );
    assert_eq!(interp.get("дл"), Some(Value::Number(2.0)));
    assert_eq!(interp.get("дб"), Some(Value::Number(8.0)));
    assert_eq!(interp.get("см"), Some(Value::Number(4.0)));
    assert_eq!(interp.get("аб"), Some(Value::Number(16.0)));
}

#[test]
fn ta_view_buffer_property_shared() {
    let interp = run_code(
        r#"
        гыы об = захуярить ОбластьБайтов(4);
        гыы а = захуярить Ц8Массив(об);
        гыы об2 = а.область;
        гыы б = захуярить Ц8Массив(об2);
        а[1] = 99;
        гыы видит = б[1];
        "#,
    );
    assert_eq!(interp.get("видит"), Some(Value::Number(99.0)));
}

#[test]
fn ta_unaligned_offset_errors() {
    let err = run_code_err(
        r#"
        гыы об = захуярить ОбластьБайтов(16);
        гыы т = захуярить Ц32Массив(об, 3, 1);
        "#,
    );
    assert!(err.message.contains("выровнено"));
}

#[test]
fn ta_negative_length_errors() {
    let err = run_code_err(
        r#"
        гыы т = захуярить Ц8Массив(-1);
        "#,
    );
    assert!(err.message.contains("неотрицательную"));
}

#[test]
fn ta_typeof_is_kind_name() {
    let interp = run_code(
        r#"
        гыы т = захуярить Ц8Массив(1);
        гыы оп = тип(т);
        "#,
    );
    assert_eq!(interp.get("оп"), Some(Value::String("Ц8Массив".to_string())));
}

#[test]
fn ta_json_serializes_as_array() {
    let interp = run_code(
        r#"
        гыы т = захуярить Ц8Массив([1, 2, 3]);
        гыы с = Жсон.вСтроку(т);
        "#,
    );
    assert_eq!(interp.get("с"), Some(Value::String("[1,2,3]".to_string())));
}

#[test]
fn ta_float64_roundtrip() {
    let interp = run_code(
        r#"
        гыы т = захуярить Др64Массив(1);
        т[0] = 3.5;
        гыы зн = т[0];
        "#,
    );
    assert_eq!(interp.get("зн"), Some(Value::Number(3.5)));
}

#[test]
fn ta_u8clamped_clamps() {
    let interp = run_code(
        r#"
        гыы т = захуярить Ц8ОграниченныйМассив(3);
        т[0] = 300;
        т[1] = -5;
        т[2] = 200;
        гыы а = т[0];
        гыы б = т[1];
        гыы в = т[2];
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::Number(255.0)));
    assert_eq!(interp.get("б"), Some(Value::Number(0.0)));
    assert_eq!(interp.get("в"), Some(Value::Number(200.0)));
}

#[test]
fn typed_array_ptr_eq_same_view() {
    let interp = run_code(
        r#"
        гыы об = захуярить ОбластьБайтов(4);
        гыы а = захуярить Ц8Массив(об);
        гыы б = захуярить Ц8Массив(об);
        гыы другойОб = захуярить ОбластьБайтов(4);
        гыы в = захуярить Ц8Массив(другойОб);
        гыы г = захуярить Ч8Массив(об);
        гыы равноСебе = а == а;
        гыы равноДругойВьюхе = а == б;
        гыы разныеОбласти = а == в;
        гыы разныеВиды = а == г;
        "#,
    );
    assert_eq!(interp.get("равноСебе"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("равноДругойВьюхе"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("разныеОбласти"), Some(Value::Boolean(false)));
    assert_eq!(interp.get("разныеВиды"), Some(Value::Boolean(false)));
}

#[test]
fn tam_nabor_copies() {
    let interp = run_code(
        r#"
        гыы т = захуярить Ц8Массив(4);
        т.набор([10, 20, 30], 1);
        гыы а = т[0];
        гыы б = т[1];
        гыы в = т[2];
        гыы г = т[3];
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::Number(0.0)));
    assert_eq!(interp.get("б"), Some(Value::Number(10.0)));
    assert_eq!(interp.get("в"), Some(Value::Number(20.0)));
    assert_eq!(interp.get("г"), Some(Value::Number(30.0)));
}

#[test]
fn tam_nabor_from_typed_array() {
    let interp = run_code(
        r#"
        гыы и = захуярить Ц8Массив([5, 6]);
        гыы т = захуярить Ц8Массив(2);
        т.набор(и);
        гыы а = т[0];
        гыы б = т[1];
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::Number(5.0)));
    assert_eq!(interp.get("б"), Some(Value::Number(6.0)));
}

#[test]
fn tam_nabor_overflow_errors() {
    let err = run_code_err(
        r#"
        гыы т = захуярить Ц8Массив(2);
        т.набор([1, 2, 3]);
        "#,
    );
    assert!(err.message.contains("не помещается"));
}

#[test]
fn ta_overflow_number_length() {
    let err = run_code_err(
        r#"
        гыы т = захуярить Др64Массив(5000000000000000000);
        "#,
    );
    assert!(err.message.contains("слишком велика"));
}

#[test]
fn ta_overflow_buffer_view_length() {
    let err = run_code_err(
        r#"
        гыы об = захуярить ОбластьБайтов(8);
        гыы т = захуярить Др64Массив(об, 0, 5000000000000000000);
        "#,
    );
    assert!(err.message.contains("слишком велика") || err.message.contains("выходит за пределы"));
}

#[test]
fn ta_overflow_set_target_offset() {
    let err = run_code_err(
        r#"
        гыы т = захуярить Ц8Массив(4);
        т.набор([1, 2], 5000000000000000000);
        "#,
    );
    assert!(err.message.contains("не помещается"));
}

#[test]
fn dv_overflow_get_offset() {
    let err = run_code_err(
        r#"
        гыы б = захуярить ОбластьБайтов(8);
        гыы в = захуярить ОбзорБайтов(б);
        в.взятьЦ32(5000000000000000000);
        "#,
    );
    assert!(err.message.contains("выходит за пределы"));
}

#[test]
fn dv_overflow_set_offset() {
    let err = run_code_err(
        r#"
        гыы б = захуярить ОбластьБайтов(8);
        гыы в = захуярить ОбзорБайтов(б);
        в.задатьДр64(5000000000000000000, 1);
        "#,
    );
    assert!(err.message.contains("выходит за пределы"));
}

#[test]
fn dv_overflow_construct_offset_length() {
    let err = run_code_err(
        r#"
        гыы б = захуярить ОбластьБайтов(8);
        гыы в = захуярить ОбзорБайтов(б, 4, 5000000000000000000);
        "#,
    );
    assert!(err.message.contains("выходит за пределы"));
}

#[test]
fn tam_podmassiv_shares_buffer() {
    let interp = run_code(
        r#"
        гыы т = захуярить Ц8Массив([1, 2, 3, 4]);
        гыы под = т.подмассив(1, 3);
        гыы дл = длина(под);
        под[0] = 99;
        гыы видитОригинал = т[1];
        "#,
    );
    assert_eq!(interp.get("дл"), Some(Value::Number(2.0)));
    assert_eq!(interp.get("видитОригинал"), Some(Value::Number(99.0)));
}

#[test]
fn tam_podmassiv_negative_indices() {
    let interp = run_code(
        r#"
        гыы т = захуярить Ц8Массив([1, 2, 3, 4, 5]);
        гыы под = т.подмассив(-2);
        гыы дл = длина(под);
        гыы а = под[0];
        гыы б = под[1];
        "#,
    );
    assert_eq!(interp.get("дл"), Some(Value::Number(2.0)));
    assert_eq!(interp.get("а"), Some(Value::Number(4.0)));
    assert_eq!(interp.get("б"), Some(Value::Number(5.0)));
}

#[test]
fn tam_srez_independent() {
    let interp = run_code(
        r#"
        гыы т = захуярить Ц8Массив([1, 2, 3, 4]);
        гыы ср = т.срез(1, 3);
        гыы дл = длина(ср);
        ср[0] = 99;
        гыы оригинал = т[1];
        гыы срезЗн = ср[0];
        "#,
    );
    assert_eq!(interp.get("дл"), Some(Value::Number(2.0)));
    assert_eq!(interp.get("оригинал"), Some(Value::Number(2.0)));
    assert_eq!(interp.get("срезЗн"), Some(Value::Number(99.0)));
}

#[test]
fn tam_srez_negative_indices() {
    let interp = run_code(
        r#"
        гыы т = захуярить Ц8Массив([1, 2, 3, 4, 5]);
        гыы ср = т.срез(-3, -1);
        гыы дл = длина(ср);
        гыы а = ср[0];
        гыы б = ср[1];
        "#,
    );
    assert_eq!(interp.get("дл"), Some(Value::Number(2.0)));
    assert_eq!(interp.get("а"), Some(Value::Number(3.0)));
    assert_eq!(interp.get("б"), Some(Value::Number(4.0)));
}

#[test]
fn tam_unknown_method_errors() {
    let err = run_code_err(
        r#"
        гыы т = захуярить Ц8Массив(1);
        т.несуществует();
        "#,
    );
    assert!(err.message.contains("нет метода"));
}

#[test]
fn dv_construct_and_properties() {
    let interp = run_code(
        r#"
        гыы б = захуярить ОбластьБайтов(8);
        гыы в = захуярить ОбзорБайтов(б);
        гыы дл = в.длинаБайт;
        гыы см = в.смещениеБайт;
        "#,
    );
    assert_eq!(interp.get("дл"), Some(Value::Number(8.0)));
    assert_eq!(interp.get("см"), Some(Value::Number(0.0)));
}

#[test]
fn dv_set_get_uint8() {
    let interp = run_code(
        r#"
        гыы б = захуярить ОбластьБайтов(4);
        гыы в = захуярить ОбзорБайтов(б);
        в.задатьЦ8(0, 255);
        гыы р = в.взятьЦ8(0);
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Number(255.0)));
}

#[test]
fn dv_set_get_int8() {
    let interp = run_code(
        r#"
        гыы б = захуярить ОбластьБайтов(4);
        гыы в = захуярить ОбзорБайтов(б);
        в.задатьЧ8(0, 255);
        гыы р = в.взятьЧ8(0);
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Number(-1.0)));
}

#[test]
fn dv_set_get_uint32_big_endian() {
    let interp = run_code(
        r#"
        гыы б = захуярить ОбластьБайтов(4);
        гыы в = захуярить ОбзорБайтов(б);
        в.задатьЦ32(0, 16909060);
        гыы р = в.взятьЦ32(0);
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Number(16909060.0)));
}

#[test]
fn dv_set_get_uint32_little_endian() {
    let interp = run_code(
        r#"
        гыы б = захуярить ОбластьБайтов(4);
        гыы в = захуярить ОбзорБайтов(б);
        в.задатьЦ32(0, 16909060, правда);
        гыы р = в.взятьЦ32(0, правда);
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Number(16909060.0)));
}

#[test]
fn dv_big_endian_vs_little_endian_differ() {
    let interp = run_code(
        r#"
        гыы б = захуярить ОбластьБайтов(4);
        гыы в = захуярить ОбзорБайтов(б);
        в.задатьЦ32(0, 16909060);
        гыы рбе = в.взятьЦ32(0);
        гыы рле = в.взятьЦ32(0, правда);
        "#,
    );
    assert_eq!(interp.get("рбе"), Some(Value::Number(16909060.0)));
    assert_eq!(interp.get("рле"), Some(Value::Number(67305985.0)));
}

#[test]
fn dv_set_get_float64() {
    let interp = run_code(
        r#"
        гыы б = захуярить ОбластьБайтов(8);
        гыы в = захуярить ОбзорБайтов(б);
        в.задатьДр64(0, 1.5);
        гыы р = в.взятьДр64(0);
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Number(1.5)));
}

#[test]
fn dv_oob_errors() {
    let err = run_code_err(
        r#"
        гыы б = захуярить ОбластьБайтов(2);
        гыы в = захуярить ОбзорБайтов(б);
        в.взятьЦ32(0);
        "#,
    );
    assert!(err.message.contains("выходит за пределы"));
}

#[test]
fn dv_shared_buffer_with_typed_array() {
    let interp = run_code(
        r#"
        гыы б = захуярить ОбластьБайтов(4);
        гыы та = захуярить Ц8Массив(б);
        гыы в = захуярить ОбзорБайтов(б);
        в.задатьЦ8(0, 42);
        гыы р = та[0];
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Number(42.0)));
}

#[test]
fn dv_with_offset_and_length() {
    let interp = run_code(
        r#"
        гыы б = захуярить ОбластьБайтов(8);
        гыы в = захуярить ОбзорБайтов(б, 4, 4);
        гыы дл = в.длинаБайт;
        гыы см = в.смещениеБайт;
        "#,
    );
    assert_eq!(interp.get("дл"), Some(Value::Number(4.0)));
    assert_eq!(interp.get("см"), Some(Value::Number(4.0)));
}
