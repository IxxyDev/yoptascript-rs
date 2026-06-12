'use strict';
console.log(null ?? 'деф');
console.log(undefined ?? 'деф');
console.log(0 ?? 'деф');
console.log('' ?? 'деф');
console.log(false ?? 'деф');

console.log(0 || 'деф');
console.log('' || 'деф');
console.log(false || 'деф');
console.log(1 || 'деф');

let а = null;
а ??= 'новое';
console.log(а);
а ??= 'ещё';
console.log(а);

let б = 5;
let счёт = 0;
function поб() { счёт++; return 99; }
б ??= поб();
console.log(б);
console.log(счёт);

let в = 0;
в ||= 7;
console.log(в);

let г = 3;
г &&= 8;
console.log(г);

let д = 0;
д &&= поб();
console.log(д);
console.log(счёт);
