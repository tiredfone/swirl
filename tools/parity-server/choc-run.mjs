const CH = process.env.CHOCOLATE || '/var/www/html/chocolate';
const placeId = +process.argv[2];
const era = +(process.argv[3] || 2012);
const ms = +(process.argv[4] || 8000);
if (!placeId) {
  console.log('usage: CHOCOLATE_DATA_DIR=/tmp/claude-1000/chocdata node choc-run.mjs <place> [era] [ms]');
  process.exit(1);
}
process.env.CHOCOLATE_DATA_DIR = process.env.CHOCOLATE_DATA_DIR || '/tmp/claude-1000/chocdata';
process.env.CHOCOLATE_KEY = 'parity-dummy';
const { Game } = await import(CH + '/server/game.js');
const sent = [];
const lines = [];
const game = new Game(placeId, {
  jobId: 'parity', kind: 'public', privateId: 0, allowGear: true, era,
  send: (cid, data) => sent.push(data),
  kick: (cid, reason) => lines.push(`[kick] ${reason}`),
  log: (line) => lines.push(line.replace(/^\[place [^\]]+\] /, '')),
});
await game.start();
await game.join(1, { id: 132, name: 'TestingAccount', membership: 0, age: 100, dev: true, perks: null, avatarType: 'R6', scale: null, animations: [] });
await new Promise((r) => setTimeout(r, ms));
game.stop();
const out = process.argv[5] || `/tmp/swirl/par/${placeId}.choc.log`;
const fs = await import('fs');
fs.writeFileSync(out, lines.join('\n') + '\n');
const kinds = {};
for (const s of sent) {
  const t = JSON.parse(s).t;
  kinds[t] = (kinds[t] || 0) + 1;
}
console.log(JSON.stringify(kinds));
process.exit(0);
