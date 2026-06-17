'use strict';
const [первый, ...остаток] = [1, 2, 3, 4];
console.log(первый);
console.log(остаток.join(","));
const [один, два, ...пусто] = [1, 2];
console.log(пусто.length);
