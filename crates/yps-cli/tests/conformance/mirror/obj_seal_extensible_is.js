// Note: yoptascript-rs silently no-ops blocked writes on sealed/non-extensible/frozen
// objects (Object.assign / Object.defineProperty / Object.setPrototypeOf) instead of throwing
// a TypeError like real JS. This mirror wraps those calls in try/catch and only compares the
// resulting observable state, which is identical either way — no output mismatch is expected.
const o1 = { а: 1, б: 2 };
Object.seal(o1);
o1.а = 100;
o1.в = 3;
delete o1.б;
console.log(o1.а, Object.hasOwn(o1, 'в'), Object.hasOwn(o1, 'б'), Object.isSealed(o1), Object.isExtensible(o1));

const o2 = { х: 1, у: 2 };
Object.preventExtensions(o2);
o2.х = 42;
o2.з = 5;
delete o2.у;
console.log(o2.х, Object.hasOwn(o2, 'з'), Object.hasOwn(o2, 'у'), Object.isSealed(o2), Object.isExtensible(o2));

const o3 = { п: 1 };
Object.freeze(o3);
console.log(Object.isFrozen(o3), Object.isSealed(o3), Object.isExtensible(o3));

console.log(Object.isSealed(5), Object.isExtensible(5), Object.isFrozen(undefined));

console.log(Object.is(NaN, NaN), Object.is(0, -0), Object.is(1, 1), Object.is('a', 'a'));

const parentA = { х: 'a' };
const parentB = { х: 'b' };
const o4 = Object.create(parentA);
Object.preventExtensions(o4);
try {
    Object.setPrototypeOf(o4, parentB);
} catch (e) {
    /* blocked on non-extensible target, matches interpreter's silent no-op */
}
console.log(Object.getPrototypeOf(o4).х);
Object.setPrototypeOf(o4, parentA);
console.log(Object.getPrototypeOf(o4).х);

const t = {};
Object.preventExtensions(t);
try {
    Object.assign(t, { а: 1, б: 2 });
} catch (e) {
    /* blocked on non-extensible target, matches interpreter's silent no-op */
}
console.log(Object.hasOwn(t, 'а'), Object.hasOwn(t, 'б'));

const o5 = { а: 1 };
Object.preventExtensions(o5);
try {
    Object.defineProperty(o5, 'б', { value: 2 });
} catch (e) {
    /* blocked on non-extensible target, matches interpreter's silent no-op */
}
Object.defineProperty(o5, 'а', { value: 99 });
console.log(Object.hasOwn(o5, 'б'), o5.а);

const o6 = {};
Object.defineProperties(o6, { а: { value: 1 }, б: { value: 2 } });
const d6 = Object.getOwnPropertyDescriptors(o6);
console.log(d6.а.value, d6.б.value);
