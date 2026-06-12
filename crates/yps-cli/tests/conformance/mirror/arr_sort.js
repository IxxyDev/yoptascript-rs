'use strict';
var numbers = [10, 9, 1, 200, 3];
console.log(numbers.sort().join(','));
console.log([10, 9, 1, 200, 3].sort((a, b) => b - a).join(','));
console.log([10, 9, 1, 200, 3].sort((a, b) => a - b).join(','));
var orig = [3, 1, 2];
var ret = orig.sort();
console.log(ret === orig);
var people = [
    { name: 'A', age: 30, order: 0 },
    { name: 'B', age: 25, order: 1 },
    { name: 'C', age: 30, order: 2 },
    { name: 'D', age: 25, order: 3 }
];
people.sort((a, b) => a.age - b.age);
people.forEach((p) => console.log(p.age + ':' + p.order));
var withUndef = [3, undefined, 1, 2];
withUndef.sort();
withUndef.forEach((e) => console.log(String(e)));
