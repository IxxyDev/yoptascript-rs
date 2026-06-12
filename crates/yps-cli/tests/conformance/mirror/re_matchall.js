'use strict';
for (const м of "a1 b2 c3".matchAll(/([a-z])(\d)/g)) {
    console.log(м[0] + " | " + м[1] + " | " + м[2] + " | " + м.index);
}
console.log("---match no g---");
const m = "abc 123".match(/(\d+)/);
console.log(m[0]);
console.log(m[1]);
console.log(m.index);
console.log("---match with g---");
const all = "a1 b2 c3".match(/\d/g);
console.log(all.length);
console.log(all.join(","));
console.log("---match no result---");
console.log("abc".match(/\d/));
console.log("---matchAll empty---");
let cnt = 0;
for (const м2 of "xyz".matchAll(/\d/g)) {
    cnt = cnt + 1;
}
console.log(cnt);
