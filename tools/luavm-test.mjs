import fs from 'node:fs';
const dir = process.argv[2] || '/var/www/swirl/dist/client';
const m = await import(dir + '/swirl_client.js');
await m.default({ module_or_path: fs.readFileSync(dir + '/swirl_client_bg.wasm') });
const t0 = performance.now();
const vm = new m.LuaVm(Date.now() / 1000);
console.log('boot ms', Math.round(performance.now() - t0));
vm.run_script(`
print("hello", _VERSION)
local ok, e = pcall(function() error("boom") end)
print(ok, e)
local p = Instance.new("Part")
p.Name = "Made"
p.Size = Vector3.new(4, 2, 6)
p.Parent = workspace
local n = 0
game:GetService("RunService").Heartbeat:Connect(function() n = n + 1 end)
spawn(function() wait(0.1) print("waited", n > 0) end)
`);
for (let i = 0; i < 20; i++) vm.tick(0.02);
console.log(vm.logs());
console.log(vm.workspace_children());
