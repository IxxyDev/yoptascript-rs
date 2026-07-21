'use strict';
class Счёт {
  static сумма = 0;
  static {
    let шаг = 5;
    this.сумма = this.сумма + шаг;
  }
  static удвой() { return this.сумма * 2; }
  static {
    this.итог = this.удвой();
  }
}
console.log(Счёт.сумма);
console.log(Счёт.итог);

class Расш extends Счёт {
  static {
    this.метка = "расш:" + this.сумма;
  }
}
console.log(Расш.сумма);
console.log(Расш.метка);
