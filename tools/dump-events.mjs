import fs from 'fs';
const CH = process.env.CHOCOLATE || '/var/www/html/chocolate';
const { memberIndex } = await import(CH + '/shared/luahost.js');
const idx = memberIndex();
const out = {};
for (const [cls, set] of idx.events) out[cls] = [...set];
fs.writeFileSync('/var/www/swirl/crates/swirl-lua/data/events.json', JSON.stringify(out));
console.log(Object.keys(out).length, 'classes with events');
