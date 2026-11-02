'use strict';
const p = Promise.resolve(10);
p.then((v) => { console.log("первый:", v); });
console.log("done");

const all = Promise.all([
    Promise.resolve(1),
    Promise.resolve(2),
    Promise.resolve(3)
]);
all.then((v) => { console.log("всех:", `[${v.join(", ")}]`); });

const x = 99;
const y = 99;
console.log(x + y);
