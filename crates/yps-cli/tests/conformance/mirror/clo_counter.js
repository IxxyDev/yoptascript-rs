'use strict';
function счёт() {
  let н = 0;
  return () => { н += 1; return н; };
}
const а = счёт();
const б = счёт();
console.log(а() + " " + а() + " " + а());
console.log(б() + " " + а());
