'use strict';
const к = 'ключ';
const о = { [к]: 1, ['а' + 'б']: 2, [3 + 4]: 9 };
console.log(о.ключ);
console.log(о.аб);
console.log(о[7]);

const н = 0;
const о2 = { [н]: 'ноль', [н + 1]: 'один' };
console.log(о2[0]);
console.log(о2[1]);

class Ф {
  constructor() {
    this['динамика'] = 42;
  }
}
const ф = new Ф();
console.log(ф.динамика);
