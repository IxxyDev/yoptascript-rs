'use strict';
var исх = [3, 1, 2];
var отсорт = исх.toSorted((a, b) => a - b);
console.log(отсорт.join(','));
console.log(исх.join(','));
var исх2 = [1, 2, 3];
var перев = исх2.toReversed();
console.log(перев.join(','));
console.log(исх2.join(','));
var исх3 = [1, 2, 3, 4, 5];
var спл = исх3.toSpliced(1, 2, 99);
console.log(спл.join(','));
console.log(исх3.join(','));
var исх4 = [1, 2, 3];
var зам = исх4.with(1, 99);
console.log(зам.join(','));
console.log(исх4.join(','));
var исх5 = [1, 2, 3];
console.log(исх5.with(-1, 99).join(','));
try { [1, 2, 3].with(5, 0); } catch (e) { console.log('with-вне-диапазона-поймано'); }
try { [1, 2, 3].with(-5, 0); } catch (e) { console.log('with-отриц-вне-поймано'); }
