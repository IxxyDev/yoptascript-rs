'use strict';
const p = Promise.resolve(10);
p.then((v) => { console.log("первый:", v); });
console.log("done");
