const fs = require('fs');
const { Scene, client_version } = require(process.argv[2] + '/swirl_client.js');
const init = fs.readFileSync(process.argv[3], 'utf8');
const s = new Scene();
const t = s.apply(init);
console.log(JSON.stringify({ client: client_version(), type: t, ready: s.ready(), era: s.era(), instances: s.instance_count(), parts: s.part_count() }));
