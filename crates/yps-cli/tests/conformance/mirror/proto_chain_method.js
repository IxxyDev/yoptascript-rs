class Base {
  greeting() { console.log("Привет от Базы"); }
  type() { console.log("База"); }
}
const obj = Object.create(Base.prototype);
obj.greeting();
obj.type();
console.log(obj instanceof Base);
console.log(Object.getPrototypeOf(obj) === Base.prototype);
