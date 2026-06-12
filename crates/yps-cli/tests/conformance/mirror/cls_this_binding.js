'use strict';
const м = { значение: 42, дай() { return this.значение; } };
console.log(м.дай());

const ф = м.дай;
try { console.log(ф()); } catch (e) { console.log('ошибка'); }

class Т {
  constructor() { this.х = 10; }
  стрела = () => this.х;
  обычный() { return this.х; }
}
const т = new Т();
console.log(т.обычный());
console.log(т.стрела());

const стр = т.стрела;
console.log(стр());

const обыч = т.обычный;
try { console.log(обыч()); } catch (e) { console.log('ошибка'); }
