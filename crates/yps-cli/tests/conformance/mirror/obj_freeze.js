// DIVERGENCE: KNOWN_DIVERGENCES.md §7.1 Object.freeze не реализован; yopta выбрасывает ошибку при вызове undefined
'use strict';
const об = { а: 1 };
Object.freeze(об);
try {
  об.а = 99;
} catch (e) {
  // в strict-mode TypeError
}
console.log(об.а);
