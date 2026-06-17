'use strict';
function внеш() {
  try {
    try { throw "перв"; }
    catch (е) { throw е + "-переброшен"; }
  } catch (е2) {
    return "внеш поймал: " + е2;
  }
}
console.log(внеш());
