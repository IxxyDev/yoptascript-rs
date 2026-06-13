class Dog {
  constructor(name) { this.name = name; }
  voice() { console.log("Гав"); }
}
const obj = Object.create(Dog.prototype);
console.log(obj instanceof Dog);
console.log(Object.getPrototypeOf(obj) === Dog.prototype);
console.log(obj.constructor === Dog);
obj.voice();
