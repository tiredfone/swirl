const fs = require('fs');
const variants = process.argv[2] ? process.argv[2].split(',') : ['r6', 'r15'];
const dir = process.env.MV || '/tmp/swirl/mv';
const tol = Number(process.env.TOL || 0.01);
let worst = 0, mism = 0, bad = 0;
for (const v of variants) {
  const js = JSON.parse(fs.readFileSync(`${dir}/js-${v}.json`));
  const rs = JSON.parse(fs.readFileSync(`${dir}/rs-${v}.json`));
  for (const k of Object.keys(js)) {
    let md = 0, mf = -1, sm = 0;
    const n = Math.min(js[k].length, rs[k].length);
    for (let i = 0; i < n; i++) {
      const a = js[k][i], b = rs[k][i];
      const d = Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2]);
      if (d > md) { md = d; mf = i; }
      if (a[3] !== b[3]) sm++;
    }
    worst = Math.max(worst, md);
    mism += sm;
    if (md > tol || sm) bad++;
    if (process.env.ALL || md > tol || sm) console.log(v, k.padEnd(14), 'max', md.toFixed(4), 'at', mf, 'states', sm);
  }
}
console.log(JSON.stringify({ worst: +worst.toFixed(5), stateMismatches: mism, scenariosOver: bad, tol }));
process.exit(worst <= tol && mism === 0 ? 0 : 1);
