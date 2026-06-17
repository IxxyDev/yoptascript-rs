'use strict';
const а = [1, 2];
const б = [0, ...а, 3];
console.log(б.join(","));
function сумма(...числа) {
  let с = 0;
  for (const н of числа) { с += н; }
  return с;
}
console.log(сумма(...б));
console.log(сумма(1, 2, 3));
const вместе = [...а, ...б];
console.log(вместе.join(","));
