'use strict';
console.log("aaa".replace(/a*/g, "-"));
console.log("abc".replace(/x*/g, "-"));
console.log("aaa".replace(/a*/, "-"));
console.log("hello".replaceAll(/(?:)/g, "."));
let cnt = 0;
for (const м of "abc".matchAll(/x*/g)) {
    cnt = cnt + 1;
}
console.log(cnt);
const parts = "abc".split(/(?:)/);
console.log(parts.length + " :: " + parts.join("|"));
console.log("a1b".replace(/\d*/g, "#"));
