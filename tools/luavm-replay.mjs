import fs from 'node:fs';
import http from 'node:http';
const dir = process.argv[2];
const tap = JSON.parse(fs.readFileSync(process.argv[3], 'utf8'));
const m = await import(dir + '/swirl_client.js');
await m.default({ module_or_path: fs.readFileSync(dir + '/swirl_client_bg.wasm') });
const get = (id) => new Promise((res) => {
  http.get({ host: '127.0.0.1', port: 80, path: '/asset/?id=' + id, headers: { Host: 'localhost' } }, (r) => {
    const b = [];
    r.on('data', (c) => b.push(c));
    r.on('end', () => res(r.statusCode === 200 ? Buffer.concat(b) : null));
  }).on('error', () => res(null));
});
const text = async (id) => {
  const b = await get(id);
  if (!b) return null;
  let t = b.toString('utf8');
  if (t.charCodeAt(0) === 0xfeff) t = t.slice(1);
  if (t.startsWith('%')) t = t.slice(t.indexOf('\n') + 1);
  if (t.startsWith('<') || /^Failed to fetch asset/.test(t)) return null;
  return t;
};
const tree = async (root) => {
  const out = new Map();
  let frontier = [root];
  for (let d = 0; d < 4 && frontier.length; d++) {
    const next = [];
    const got = await Promise.all(frontier.filter((i) => !out.has(i)).map(async (i) => [i, await text(i)]));
    for (const [i, s] of got) {
      out.set(i, s);
      if (!s) continue;
      for (const re of [/AddCoreScript\(\s*(\d+)/g, /AddStarterScript\(\s*(\d+)/g, /RegisterLibrary\([^,]+,\s*["']?(\d+)/g]) for (const x of s.matchAll(re)) next.push(+x[1]);
    }
    frontier = next;
  }
  return out;
};
const vm = new m.LuaVm(Date.now() / 1000);
vm.set_run(true);
let n = 0;
const dt = +(process.env.DT || 1 / 60);
const all = [];
for (const raw of tap) {
  const t = JSON.parse(raw).t;
  if (t === 'init' || t === 'rep') vm.apply(raw);
  if (t === 'rep') {
    for (const id of JSON.parse(vm.take_wanted())) for (const [k, v] of [...(await tree(id))].reverse()) vm.provide_core_source(k, v);
    vm.update(dt, 0, 0, false);
    n++;
    for (const l of JSON.parse(vm.logs())) all.push(l);
  }
}
for (let i = 0; i < +(process.env.EXTRA || 120); i++) {
  vm.update(dt, 0, 0, false);
  for (const l of JSON.parse(vm.logs())) all.push(l);
}
console.log(vm.class_counts("CoreGui"), vm.children_of("CoreGui.RobloxGui.CurrentLoadout.Slot1"));
for (const l of all.slice(0, 8)) console.log(l.kind, l.msg.slice(0, 200));
if (process.env.LUA) {
  vm.run_script(process.env.LUA);
  vm.update(dt, 0, 0, false);
  for (const l of JSON.parse(vm.logs())) console.log('LUA', l.kind, l.msg.slice(0, 300));
}
