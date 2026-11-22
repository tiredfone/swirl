import fs from 'fs';
const CH = process.env.CHOCOLATE || '/var/www/html/chocolate';
const out = [];
const re = /def\('([^']+)', '([^']+)', \((?:[a-z_]+|\[[^\]]*\]|\([^)]*\)|, )*\) => (null|false|true|-?\d+(?:\.\d+)?|'[^'\\]*'|T\.list\(\[\]\)|T\.dict\(\{\}\))\);/g;
for (const f of ['shared/luahost.js', 'shared/api2018.js']) {
  const src = fs.readFileSync(CH + '/' + f, 'utf8');
  let m;
  while ((m = re.exec(src))) {
    let v = m[3];
    let value;
    if (v === 'null') value = null;
    else if (v === 'false') value = false;
    else if (v === 'true') value = true;
    else if (v.startsWith("'")) value = v.slice(1, -1);
    else if (v.startsWith('T.list')) value = [];
    else if (v.startsWith('T.dict')) value = {};
    else value = Number(v);
    out.push({ names: m[1].split(' '), cls: m[2], value });
  }
}
fs.writeFileSync('/var/www/swirl/crates/swirl-lua/data/stubs.json', JSON.stringify(out));
console.log(out.length, 'stub methods');
