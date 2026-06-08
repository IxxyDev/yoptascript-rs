#!/usr/bin/env node
'use strict';

// Офлайн-оракул для conformance-батареи YoptaScript-rs.
// НЕ запускается в CI. Разработчик запускает вручную:
//   node tools/gen-golden.js
// Скрипт печатает эталонные (Node) значения для коэрсивных кейсов, чтобы
// сверить с golden-файлами после шага 3 (abstract_equals + Add через ToPrimitive).
// Это не транспайлер YoptaScript: здесь вручную закодированы JS-эквиваленты
// выражений из cases/coercion_*.yop в том же порядке вывода.

function section(name, lines) {
  console.log(`===== ${name} =====`);
  for (const v of lines) console.log(v);
  console.log('');
}

// cases/coercion_equality.yop
section('coercion_equality', [
  1 == '1',
  1 === '1',
  null == undefined,
  null === undefined,
  true == 1,
  0 == false,
  '' == 0,
  undefined == 0,
  null == 0,
  'abc' == 'abc',
  1 != '1',
  1 !== '1',
].map(String));

// cases/coercion_add.yop
section('coercion_add', [
  1 + 2,
  'a' + 'b',
  'n=' + 1,
  1 + 'x',
  '' + true,
  '' + null,
  '' + undefined,
  10 + 5 + 'px',
  'px' + 10 + 5,
].map(String));

// cases/coercion_stringify.yop
// число(x) == Number(x); строка(x) == String(x); "+" объект -> ToPrimitive(String) -> [object Object]
const ob = { a: 1 };
const mas = [1, 2, 3];
section('coercion_stringify', [
  'об=' + ob,
  'мас=' + mas,
  Number('42'),
  Number('  7  '),
  Number(true),
  Number(null),
  String(42),
  String(true),
].map(String));
