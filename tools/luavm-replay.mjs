import fs from 'node:fs';
const dir = process.argv[2];
const tap = JSON.parse(fs.readFileSync(process.argv[3], 'utf8'));
const m = await import(dir + '/swirl_client.js');
await m.default({ module_or_path: fs.readFileSync(dir + '/swirl_client_bg.wasm') });
const vm = new m.LuaVm(Date.now() / 1000);
vm.set_run(true);
let n = 0;
for (const raw of tap) {
  const t = JSON.parse(raw).t;
  if (t === 'init' || t === 'rep') vm.apply(raw);
  if (t === 'rep') {
    vm.update(+(process.env.DT || 1 / 60), 0, 0, false);
    n++;
    if (n < 16 || n % 20 === 0) console.log(n, vm.own_state());
  }
}
console.log(vm.logs());
