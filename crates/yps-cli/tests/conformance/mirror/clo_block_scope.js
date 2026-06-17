'use strict';
let х = 1;
{
  let х = 2;
  console.log(х);
  {
    let х = 3;
    console.log(х);
  }
  console.log(х);
}
console.log(х);
const к = 42;
try { к = 99; } catch (e) { console.log("const защищён"); }
console.log(к);
