'use strict';
console.log([1, 2, 3, 4].reduce((a, b) => a + b));
console.log([1, 2, 3, 4].reduce((a, b) => a + b, 100));
console.log([5].reduce((a, b) => a + b));
console.log([].reduce((a, b) => a + b, 0));
console.log([1, 2, 3, 4].reduceRight((a, b) => a + '-' + b));
console.log([1, 2, 3, 4].reduceRight((a, b) => a + b, 100));
console.log([5].reduceRight((a, b) => a + b));
console.log([].reduceRight((a, b) => a + b, 7));
try { [].reduce((a, b) => a + b); } catch (e) { console.log('reduce-пусто-поймано'); }
try { [].reduceRight((a, b) => a + b); } catch (e) { console.log('reduceRight-пусто-поймано'); }
