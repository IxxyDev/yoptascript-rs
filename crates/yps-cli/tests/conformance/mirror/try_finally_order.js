'use strict';
const лог = [];
function ф(бросать) {
  try {
    лог.push("try");
    if (бросать) { throw "бум"; }
  } catch (е) {
    лог.push("catch:" + е);
  } finally {
    лог.push("finally");
  }
}
ф(false);
ф(true);
console.log(лог.join(","));
