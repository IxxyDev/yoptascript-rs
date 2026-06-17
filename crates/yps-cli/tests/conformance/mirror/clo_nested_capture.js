'use strict';
function снаружи(а) {
  function внутри(б) { return а + б; }
  return внутри;
}
const доб5 = снаружи(5);
const доб10 = снаружи(10);
console.log(доб5(3) + " " + доб10(3) + " " + доб5(100));
