// DIVERGENCE: KNOWN_DIVERGENCES.md §5.1 строки в скалярных значениях Unicode, а не UTF-16
'use strict';
console.log("Привет Hello".toUpperCase());
console.log("ПРИВЕТ HELLO".toLowerCase());
console.log("ЁжИк Test".toLowerCase());
console.log("ёжик test".toUpperCase());
console.log("abcDEF".length);
console.log("привет".length);
console.log("a😀b".length);
console.log("😀".length);
console.log("café".length);
console.log("a😀b".at(1));
console.log("a😀b".slice(1, 2));
console.log("a😀b".charAt(1));
