import { CLASSES, ENUMS } from '/var/www/html/chocolate/shared/classes.js';
import * as M from '/var/www/html/chocolate/shared/classes.js';
const classes = [];
for (const c of CLASSES.values()) {
  const own = [];
  for (const [n, p] of c.own) own.push({ name: n, type: p.type, def: p.def, enumType: p.enumType });
  classes.push({ name: c.name, base: c.base, abstract: c.abstract, service: c.service, nocreate: c.nocreate, era: c.era, aliases: c.aliases, own });
}
const era2018 = [];
for (const c of CLASSES.values()) for (const n of c.all.keys()) if (M.propEra(c.name, n) === 2018) era2018.push(c.name + '.' + n);
console.log(JSON.stringify({ classes, enums: ENUMS, replicatedServices: M.REPLICATED_SERVICES, era2018 }));
