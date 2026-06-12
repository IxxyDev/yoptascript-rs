'use strict';
class Мат {
  static ПИ = 3.14;
  static кв(х) { return х * х; }
  static описание() { return 'Мат(' + this.ПИ + ')'; }
}
console.log(Мат.ПИ);
console.log(Мат.кв(4));
console.log(Мат.описание());

class РасшМат extends Мат {
  static куб(х) { return х * х * х; }
}
console.log(РасшМат.ПИ);
console.log(РасшМат.кв(3));
console.log(РасшМат.куб(2));
