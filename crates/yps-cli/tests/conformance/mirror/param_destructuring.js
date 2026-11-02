'use strict';
function f({ x, y }) {
  console.log(x, y);
}
f({ x: 1, y: 2 });

function g([a, b]) {
  console.log(a, b);
}
g([10, 20]);

function h({ x = 5 } = {}) {
  console.log(x);
}
h();
h({ x: 99 });

function multi({ name }, [first]) {
  console.log(name, first);
}
multi({ name: "Иван" }, ["привет"]);

const arrow = ({ a, b }) => a + b;
console.log(arrow({ a: 3, b: 4 }));
