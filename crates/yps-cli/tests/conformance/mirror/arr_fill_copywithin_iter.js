'use strict';
console.log([1, 2, 3, 4, 5].fill(0).join(','));
console.log([1, 2, 3, 4, 5].fill(0, 1, 3).join(','));
console.log([1, 2, 3, 4, 5].fill(0, -3).join(','));
console.log([1, 2, 3, 4, 5].copyWithin(0, 3).join(','));
console.log([1, 2, 3, 4, 5].copyWithin(1, 3).join(','));
console.log([1, 2, 3, 4, 5].copyWithin(-2, -3, -1).join(','));

const f = ['a', 'b', 'c'];
for (const pair of f.entries()) {
    console.log(pair[0], pair[1]);
}
for (const k of f.keys()) {
    console.log(k);
}
for (const v of f.values()) {
    console.log(v);
}
console.log([...f.keys()].join(','));
console.log([...f.values()].join(','));
