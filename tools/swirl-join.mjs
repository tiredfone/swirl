import path from 'path';
import { createRequire } from 'module';
const CH = '/var/www/html/chocolate';
const require = createRequire(CH + '/package.json');
const WebSocket = require('ws');
const { DataModel } = await import(CH + '/shared/instance.js');
const { applyRecords } = await import(CH + '/shared/codec.js');
const port = +process.argv[2];
if (!port || port === 4201 || port === 4202) {
  console.log('usage: node tools/swirl-join.mjs <spare port>');
  process.exit(1);
}
const ws = new WebSocket('ws://127.0.0.1:' + port + '/ws?ticket=dev');
const got = [];
await new Promise((resolve, reject) => {
  const timer = setTimeout(() => reject(new Error('no init')), 10000);
  ws.on('open', () => ws.send(JSON.stringify({ t: 'ping', x: 7 })));
  ws.on('message', (d) => {
    const m = JSON.parse(d.toString());
    got.push(m.t);
    if (m.t === 'init') {
      const dm = new DataModel('client');
      dm.era = m.era;
      const fresh = applyRecords(dm, m.snap);
      const ws_ = dm.services.get('Workspace');
      const all = [];
      ws_.descendants(all);
      const parts = all.filter((x) => x.isA('BasePart'));
      console.log(JSON.stringify({ era: m.era, records: m.snap.length, fresh: fresh.length, workspaceDescendants: all.length, parts: parts.length, you: dm.byId.get(m.you) && dm.byId.get(m.you).ClassName, user: m.user, services: [...dm.services.keys()].length }));
      const p = parts[0];
      if (p) console.log('first part', p.Name, JSON.stringify(p.props.get('Size')), JSON.stringify(p.props.get('CFrame')));
    }
    if (m.t === 'pong') {
      clearTimeout(timer);
      resolve();
    }
  });
  ws.on('error', reject);
});
console.log('messages', got.join(','));
ws.close();
process.exit(0);
