import fs from 'fs';
const CH = process.env.CHOCOLATE || '/var/www/html/chocolate';
const { memberIndex } = await import(CH + '/shared/luahost.js');
const { LUA_CLASS } = await import(CH + '/shared/api2018.js');
const idx = memberIndex();
const methods = {};
for (const [cls, set] of idx.methods) methods[cls] = [...set];
const luaClass = {};
for (const [name, classes] of Object.entries(LUA_CLASS)) luaClass[name] = classes;
for (const [cls, set] of idx.luaMethods ? Object.entries(idx.luaMethods) : []) {
  for (const name of set) {
    if (!luaClass[name]) luaClass[name] = [];
    if (!luaClass[name].includes(cls)) luaClass[name].push(cls);
  }
}
for (const n of ['TweenPosition', 'tweenPosition', 'TweenSize', 'tweenSize', 'TweenSizeAndPosition', 'tweenSizeAndPosition']) luaClass[n] = ['GuiObject'];
const computed = {};
for (const [cls, set] of idx.computed) computed[cls] = [...set];
fs.writeFileSync('/var/www/swirl/crates/swirl-lua/data/members.json', JSON.stringify({ methods, luaClass, computed }));
console.log(Object.keys(methods).length, 'classes with methods', Object.keys(luaClass).length, 'lua members');
