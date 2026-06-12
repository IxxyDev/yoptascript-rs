'use strict';
class А {
  constructor() { this.а = 'a'; }
  метА() { return 'А'; }
}
class Б extends А {
  constructor() { super(); this.б = 'b'; }
  метБ() { return super.метА() + '+Б'; }
}
class В extends Б {
  constructor() { super(); this.в = 'c'; }
}
const в = new В();
console.log(в.а);
console.log(в.б);
console.log(в.в);
console.log(в.метБ());
console.log(в instanceof А);
console.log(в instanceof Б);
console.log(в instanceof В);
