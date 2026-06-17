'use strict';
const лог = [];
function поб(м, в) { лог.push(м); return в; }
const р1 = поб("a", false) && поб("b", true);
const р2 = поб("c", true) || поб("d", true);
console.log(лог.join(","));
console.log(р1 + " " + р2);
