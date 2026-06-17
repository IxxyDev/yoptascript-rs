'use strict';
function ф() {
  try { return "из-try"; }
  finally { return "из-finally"; }
}
console.log(ф());
function г() {
  try { return "ок"; }
  finally { console.log("finally-побочный"); }
}
console.log(г());
