'use strict';
const фс = [];
for (let и = 0; и < 3; и += 1) {
  фс.push(() => и);
}
console.log(фс[0]() + " " + фс[1]() + " " + фс[2]());
const гс = [];
for (const х of [10, 20, 30]) {
  гс.push(() => х);
}
console.log(гс[0]() + " " + гс[1]() + " " + гс[2]());
