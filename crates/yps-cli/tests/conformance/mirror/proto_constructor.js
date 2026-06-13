class Animal {
  constructor(name) { this.name = name; }
}
class Dog extends Animal {
  constructor(name) { super(name); }
}
const dog = new Dog("Bobik");
console.log(dog.constructor === Dog);
console.log(Dog.prototype.constructor === Dog);
console.log(Animal.prototype.constructor === Animal);
console.log(Dog.prototype === Dog.prototype);
