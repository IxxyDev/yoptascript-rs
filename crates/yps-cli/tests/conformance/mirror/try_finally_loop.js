'use strict';
const лог = [];
for (let и = 0; и < 5; и += 1) {
  try {
    if (и === 2) { break; }
    лог.push("б:" + и);
  } finally {
    лог.push("f:" + и);
  }
}
console.log(лог.join(","));
const лог2 = [];
for (let к = 0; к < 3; к += 1) {
  try {
    if (к === 1) { continue; }
    лог2.push("т:" + к);
  } finally {
    лог2.push("ф:" + к);
  }
}
console.log(лог2.join(","));
