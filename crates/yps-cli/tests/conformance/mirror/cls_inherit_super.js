'use strict';
class Зверь {
  constructor(имя) { this.имя = имя; }
  голос() { return '...'; }
  опис() { return 'Зверь:' + this.имя; }
}
class Пёс extends Зверь {
  constructor(имя) { super(имя); }
  голос() { return super.голос() + 'гав'; }
}
const п = new Пёс('Рекс');
console.log(п.имя);
console.log(п.голос());
console.log(п.опис());
console.log(п instanceof Пёс);
console.log(п instanceof Зверь);
