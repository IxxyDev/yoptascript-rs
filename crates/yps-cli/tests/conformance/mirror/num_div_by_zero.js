// DIVERGENCE: KNOWN_DIVERGENCES.md §4.4 division by zero throws a runtime error instead of yielding Infinity/NaN
'use strict';

function check(f) {
  try {
    return String(f());
  } catch (e) {
    return 'ошибка';
  }
}
console.log(check(() => 1 / 0));
console.log(check(() => -1 / 0));
console.log(check(() => 0 / 0));
console.log(check(() => 6 / 2));
console.log(check(() => 7 / 2));
