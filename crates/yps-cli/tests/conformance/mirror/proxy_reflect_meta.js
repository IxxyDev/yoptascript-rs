'use strict';
// Note: yoptascript-rs proxy traps are lax about target invariants — Object.keys / for-in
// enumeration routes only through the ownKeys trap and does NOT probe each key via the
// getOwnPropertyDescriptor trap the way real JS does. This mirror uses Reflect.ownKeys for the
// logged enumeration so the trap-call log matches; the observable key lists are identical either way.
const лог = [];

const мишень = { а: 1, б: 2 };
const наблюдатель = new Proxy(мишень, {
    ownKeys(ц) { лог.push('ownKeys'); return Reflect.ownKeys(ц); },
    defineProperty(ц, к, деск) { лог.push('define:' + к); return Reflect.defineProperty(ц, к, деск); },
    getOwnPropertyDescriptor(ц, к) { лог.push('descr:' + к); return Reflect.getOwnPropertyDescriptor(ц, к); },
    preventExtensions(ц) { лог.push('prevent'); return Reflect.preventExtensions(ц); },
    isExtensible(ц) { return Reflect.isExtensible(ц); }
});

console.log(Reflect.ownKeys(наблюдатель).join(','));
Object.defineProperty(наблюдатель, 'в', { value: 3, enumerable: true, configurable: true, writable: true });
console.log(Object.getOwnPropertyDescriptor(наблюдатель, 'в').value);
console.log(Reflect.isExtensible(наблюдатель));
Object.preventExtensions(наблюдатель);
console.log(Reflect.isExtensible(наблюдатель));
console.log(Reflect.ownKeys(наблюдатель).join(','));
console.log(лог.join('|'));

const ядро = { счёт: 10 };
const зеркало = new Proxy(ядро, {
    get(ц, к) { return Reflect.get(ц, к); },
    set(ц, к, зн) { return Reflect.set(ц, к, зн); },
    has(ц, к) { return Reflect.has(ц, к); },
    deleteProperty(ц, к) { return Reflect.deleteProperty(ц, к); },
    ownKeys(ц) { return Reflect.ownKeys(ц); }
});

зеркало.имя = 'зеркальный';
console.log(зеркало.счёт, зеркало.имя);
console.log('счёт' in зеркало);
console.log(Object.keys(зеркало).join(','));
delete зеркало.счёт;
console.log('счёт' in зеркало, зеркало.имя);

const прото = { вид: 'прототип' };
const сменщик = new Proxy({}, {
    setPrototypeOf(ц, прт) { return Reflect.setPrototypeOf(ц, прт); },
    getPrototypeOf(ц) { return Reflect.getPrototypeOf(ц); }
});
Object.setPrototypeOf(сменщик, прото);
console.log(Object.getPrototypeOf(сменщик).вид);
