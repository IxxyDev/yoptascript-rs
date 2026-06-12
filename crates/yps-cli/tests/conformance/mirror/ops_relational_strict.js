// DIVERGENCE: KNOWN_DIVERGENCES.md §4.2 relational operators require two numbers (no coercion); non-numeric operands throw
'use strict';

function check(f) {
  try {
    return String(f());
  } catch (e) {
    return 'ошибка';
  }
}
console.log(check(() => '10' < '9'));
console.log(check(() => '10' < 9));
console.log(check(() => null < 1));
console.log(check(() => undefined < 1));
console.log(check(() => true < 2));
console.log(check(() => 5 < 3));
console.log(check(() => 3 <= 3));
console.log(check(() => 7 > 2));
console.log(check(() => 2 >= 9));
