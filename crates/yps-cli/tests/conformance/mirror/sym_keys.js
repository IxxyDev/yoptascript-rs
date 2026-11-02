'use strict';
const sym = Symbol("ключ");
const obj = {};
obj[sym] = 42;
console.log(obj[sym]);

const sym2 = Symbol.iterator;
const obj2 = { [sym2]: "итератор" };
console.log(obj2[sym2]);

const s1 = Symbol("x");
const s2 = Symbol("x");
console.log(s1 === s2);
console.log(s1 === s1);

const methodSym = Symbol("метод");
const o3 = { [methodSym]() { return "вызван"; } };
console.log(o3[methodSym]());

const keys = Object.keys(obj);
console.log(keys.length);
