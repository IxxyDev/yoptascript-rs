'use strict';
try { console.log(String(1n + 1)); } catch (е) { console.log("смеш-плюс"); }
try { console.log(String(2n * 3)); } catch (е) { console.log("смеш-умнож"); }
console.log(2n == 2);
console.log(2n === 2);
