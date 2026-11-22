const fs = require('fs');
const WebSocket = require('/tmp/claude-1000/pp/node_modules/ws');
const [, , port, place, secs] = process.argv;
if (!port || port === '4201' || port === '4202') {
  console.log('usage: node tools/swirl-ticket-join.cjs <spare port> <place> [seconds]');
  process.exit(1);
}
const acct = JSON.parse(fs.readFileSync('/home/fone/.config/solar/testing-account.json'));
const base = 'http://127.0.0.1';
const pp = require('/tmp/claude-1000/pp/node_modules/puppeteer-core');
async function getTicket() {
  const b = await pp.launch({ executablePath: '/usr/bin/chromium', headless: 'new', args: ['--no-sandbox'] });
  const pg = await b.newPage();
  await pg.goto('http://127.0.0.1/login', { waitUntil: 'networkidle2' });
  const st = await pg.evaluate(async (u, pw) => {
    const t = document.querySelector('input[name=_csrf]').value;
    const r = await fetch('/api/login', { method: 'POST', body: new URLSearchParams({ _csrf: t, Username: u, Password: pw }), credentials: 'same-origin' });
    return r.status;
  }, acct.username, acct.password);
  await pg.goto('http://127.0.0.1/game/' + place, { waitUntil: 'domcontentloaded' });
  const out = await pg.evaluate(async (pl) => {
    const m = document.querySelector('meta[name=csrf-token]');
    const t = m ? m.content : (document.querySelector('input[name=_csrf]') || {}).value;
    const r = await fetch('/api/chocolate/ticket', { method: 'POST', headers: { 'content-type': 'application/json', 'X-CSRF-Token': t || '' }, body: JSON.stringify({ placeId: pl }), credentials: 'same-origin' });
    return { status: r.status, body: await r.text() };
  }, +place);
  await b.close();
  let j = {};
  try { j = JSON.parse(out.body); } catch (e) {}
  console.log('login', st, 'ticket status', out.status, Object.keys(j).join(','));
  return j;
}
(async () => {
  const tj = await getTicket();
  if (!tj.ticket) { console.log(JSON.stringify({ ...tj, ticket: undefined }).slice(0, 300)); process.exit(1); }
  const ws = new WebSocket('ws://127.0.0.1:' + port + '/ws?ticket=' + tj.ticket);
  const counts = {};
  const prints = [];
  ws.on('message', (d) => {
    const m = JSON.parse(d.toString());
    counts[m.t] = (counts[m.t] || 0) + 1;
    if (m.t === 'init') console.log('init', m.snap ? m.snap.length : 0, 'records, era', m.era, 'user', m.user);
    if (m.t === 'kick') console.log('kick', m.m);
    if (m.t === 'mktr' || m.t === 'pver') console.log(JSON.stringify(m).slice(0, 300));
    if (m.t === 'sys' || m.t === 'out' || m.t === 'log') prints.push(JSON.stringify(m).slice(0, 160));
  });
  ws.on('close', () => console.log('closed'));
  ws.on('open', () => { console.log('open'); if (process.env.SEND) setTimeout(() => ws.send(process.env.SEND), 2500); });
  setTimeout(() => { console.log(JSON.stringify(counts)); console.log(prints.slice(0, 15).join('\n')); ws.close(); setTimeout(() => process.exit(0), 300); }, (+secs || 6) * 1000);
})();
