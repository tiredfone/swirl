const WS = (await import('/var/www/html/chocolate/node_modules/ws/wrapper.mjs')).default;
const port = +process.argv[2];
const ws = new WS('ws://127.0.0.1:' + port + '/ws?ticket=dev');
ws.on('message', (d) => {
  const m = JSON.parse(d.toString());
  if (m.t === 'init') {
    console.log(JSON.stringify(m));
    process.exit(0);
  }
});
