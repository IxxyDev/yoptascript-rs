// DIVERGENCE: KNOWN_DIVERGENCES.md §4.3 unary + and - require a number (no ToNumber coercion); other types throw
'use strict';

function check(f) {
  try {
    return String(f());
  } catch (e) {
    return 'ошибка';
  }
}
console.log(check(() => +'5'));
console.log(check(() => +''));
console.log(check(() => +'abc'));
console.log(check(() => +true));
console.log(check(() => +null));
console.log(check(() => +undefined));
console.log(check(() => -'3'));
console.log(check(() => +42));
console.log(check(() => -42));
