'use strict';
const а = null;
console.log(а ?? "дефолт");
console.log(0 ?? "нет");
console.log(undefined ?? "тоже дефолт");
const оц = 85;
console.log(оц >= 90 ? "A" : оц >= 80 ? "B" : "C");
