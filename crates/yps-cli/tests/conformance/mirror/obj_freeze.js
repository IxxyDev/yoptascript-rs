const об = { а: 1 };
Object.freeze(об);
об.а = 99;
об.б = 2;
console.log(об.а);
console.log(об.б);
console.log(Object.isFrozen(об));
