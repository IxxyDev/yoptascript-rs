'use strict';
const об = {
  _н: 5,
  get н() { return this._н * 2; },
  set н(в) { this._н = в + 1; }
};
console.log(об.н);
об.н = 10;
console.log(об._н);
console.log(об.н);

class Ячейка {
  constructor() { this._ц = 0; }
  get удвоить() { return this._ц * 2; }
  set уст(в) { this._ц = в; }
}
const я = new Ячейка();
я.уст = 4;
console.log(я.удвоить);
я.уст = 7;
console.log(я.удвоить);
