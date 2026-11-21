import { ENUMS } from '/var/www/html/chocolate/shared/classes.js';
import { BRICK, brickPalette } from '/var/www/html/chocolate/shared/brickcolor.js';
const luaStr = (s) => '"' + String(s).replace(/\\/g, '\\\\').replace(/"/g, '\\"').replace(/\n/g, '\\n') + '"';
const b = [];
for (const c of BRICK.values()) b.push(`[${c.n}]={name=${luaStr(c.name)},r=${c.r},g=${c.g},b=${c.b}}`);
const e = [];
for (const [name, items] of Object.entries(ENUMS)) e.push(`[${luaStr(name)}]={${Object.entries(items).map(([k, v]) => `[${luaStr(k)}]=${v}`).join(',')}}`);
process.stdout.write(`return {bricks={${b.join(',')}},enums={${e.join(',')}},palette={${brickPalette().join(',')}}}`);
