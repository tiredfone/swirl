const fs = require('fs');
const dir = process.argv[2] || '/tmp/swirl/anim';
const a = JSON.parse(fs.readFileSync(dir + '/js.json'));
const b = JSON.parse(fs.readFileSync(dir + '/rs.json'));
let worst = 0;
let mismatch = 0;
let first = null;
for (let f = 0; f < a.length; f++) {
  for (let j = 0; j < a[f].length; j++) {
    const x = a[f][j][1];
    const y = b[f][j][1];
    if (!x !== !y) {
      mismatch++;
      if (!first) first = [f, j, x, y];
      continue;
    }
    if (!x) continue;
    for (let k = 0; k < 12; k++) {
      const d = Math.abs(x[k] - y[k]);
      if (d > worst) {
        worst = d;
        if (d > 1e-3 && !first) first = [f, j, x, y];
      }
    }
  }
}
console.log(JSON.stringify({ frames: a.length, worst, mismatch, first }));
process.exit(worst < 1e-3 && mismatch === 0 ? 0 : 1);
