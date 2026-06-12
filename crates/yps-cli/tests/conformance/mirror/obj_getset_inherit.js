'use strict';
class База {
  get вид() { return 'база'; }
}
class Под extends База {}
const п = new Под();
console.log(п.вид);

class Ж {
  constructor() { this._х = 10; }
  get х() { return this._х; }
  set х(в) { this._х = в; }
}
class Д extends Ж {
  constructor() { super(); }
}
const д = new Д();
console.log(д.х);
д.х = 99;
console.log(д.х);
