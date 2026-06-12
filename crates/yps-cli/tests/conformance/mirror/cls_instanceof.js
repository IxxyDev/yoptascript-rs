'use strict';
class Ж {}
class П extends Ж {}
class Д extends П {}
class Др {}

const д = new Д();
console.log(д instanceof Д);
console.log(д instanceof П);
console.log(д instanceof Ж);
console.log(д instanceof Др);

console.log(5 instanceof Д);
console.log(null instanceof Д);
console.log(undefined instanceof Д);
console.log('x' instanceof Д);

const ж = new Ж();
console.log(ж instanceof П);
