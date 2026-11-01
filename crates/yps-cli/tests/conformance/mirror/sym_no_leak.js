'use strict';
const sym = Symbol("сек");
const src = {};
src[sym] = 99;

const via_spread = { ...src };
console.log(Object.keys(via_spread).length);

const target = {};
Object.assign(target, src);
console.log(Object.keys(target).length);

console.log(via_spread[sym]);
console.log(target[sym]);
