const WebSocket = require('/var/www/html/chocolate/node_modules/ws');
const { Scene } = require(process.argv[2] + '/swirl_client.js');
const ws = new WebSocket('ws://127.0.0.1:' + process.argv[3] + '/ws?ticket=dev');
const s = new Scene();
ws.on('message', (d) => {
  const t = s.apply(d.toString());
  if (t === 'init') {
    console.log(JSON.stringify({ ready: s.ready(), instances: s.instance_count(), parts: s.part_count() }));
    process.exit(s.part_count() > 0 ? 0 : 1);
  }
});
setTimeout(() => process.exit(2), 8000);
