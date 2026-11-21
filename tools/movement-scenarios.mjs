export async function runAll(load, RAPIER) {
  const { DataModel } = await load('shared/instance.js');
  const { Physics } = await load('shared/physics.js');
  const { buildCharacter, Controller, CharacterProxy } = await load('shared/humanoid.js');
  const C = await load('shared/cframe.js');
  await RAPIER.init();

  const scene = (parts) => {
    const dm = new DataModel('client');
    const ws = dm.service('Workspace');
    for (const [name, pos, size, cls] of parts) {
      const p = dm.create(cls || 'Part', null);
      p.props.set('Name', name);
      p.props.set('Anchored', true);
      p.props.set('Size', size);
      p.props.set('CFrame', C.cf(pos[0], pos[1], pos[2]));
      dm.setParent(p, ws);
    }
    const ph = new Physics(dm, RAPIER, { simulate: false, excluded: () => false });
    ph.attachWorkspace(ws);
    const { model, humanoid } = buildCharacter(dm, 'Tester', [0, 4, 0]);
    const ctrl = new Controller(ph, model, humanoid);
    ctrl.teleport(C.cf(0, 4, 0));
    return { ph, ctrl, model };
  };

  const trace = [];
  const run = (s, seconds, input) => {
    const dt = 1 / 60;
    for (let i = 0; i < Math.round(seconds * 60); i++) {
      const r = s.ctrl.update(dt, typeof input === 'function' ? input(i * dt) : input);
      s.ph.world.step();
      trace.push([+r.cf[0].toFixed(5), +r.cf[1].toFixed(5), +r.cf[2].toFixed(5), r.state]);
    }
  };

  const fwd = { move: [0, 0, -1], jump: false };
  const back = { move: [0, 0, 1], jump: false };
  const idle = { move: [0, 0, 0], jump: false };
  const floor = ['Floor', [0, -0.5, 0], [60, 1, 60]];
  const out = {};
  const scenario = (name, fn) => {
    trace.length = 0;
    fn();
    out[name] = trace.slice();
  };

  scenario('seam', () => {
    const s = scene([['A', [0, -0.5, 0], [40, 1, 40]], ['B', [0, -0.5, -40], [40, 1, 40]]]);
    run(s, 1, idle);
    run(s, 3, fwd);
  });
  scenario('lip', () => {
    const s = scene([['A', [0, -0.5, 0], [40, 1, 40]], ['B', [0, -0.45, -40], [40, 1, 40]]]);
    run(s, 1, idle);
    run(s, 3, fwd);
  });
  scenario('plate', () => {
    const s = scene([floor, ['Plate', [0, 0.2, -10], [8, 0.4, 4]]]);
    run(s, 1, idle);
    run(s, 1.2, fwd);
  });
  scenario('step', () => {
    const s = scene([floor, ['Brick', [0, 0.6, -10], [8, 1.2, 4]]]);
    run(s, 1, idle);
    run(s, 1, fwd);
  });
  scenario('stairs', () => {
    const s = scene([floor, ...[0, 1, 2, 3, 4, 5].map((k) => ['Stair' + k, [0, 0.4 * (k + 1) - 0.2, -8 - k * 2.5], [8, 0.4 * (k + 1), 2.5]])]);
    run(s, 1, idle);
    run(s, 4, fwd);
  });
  scenario('wall', () => {
    const s = scene([floor, ['Wall', [0, 1.2, -10], [8, 2.4, 4]]]);
    run(s, 1, idle);
    run(s, 2, fwd);
  });
  scenario('tallwall', () => {
    const s = scene([floor, ['Tall', [0, 10, -8], [12, 20, 1]]]);
    run(s, 1, idle);
    run(s, 2, fwd);
  });
  scenario('hop', () => {
    const s = scene([floor]);
    run(s, 1, idle);
    run(s, 3, { move: [0, 0, 0], jump: true });
  });
  scenario('gap', () => {
    const s = scene([['A', [0, -0.5, 0], [20, 1, 20]], ['B', [0, -0.5, -23], [20, 1, 20]]]);
    run(s, 1, idle);
    run(s, 4, (t) => ({ move: [0, 0, -1], jump: t > 0.7 && t < 0.78 }));
  });
  scenario('fall', () => {
    const s = scene([['Floor', [0, -0.5, -50], [60, 1, 60]]]);
    run(s, 0.5, idle);
    run(s, 2, fwd);
  });
  scenario('drop', () => {
    const s = scene([floor, ['Tower', [0, 15, 0], [10, 30, 10]]]);
    run(s, 1, idle);
    run(s, 3, fwd);
  });
  scenario('ledge', () => {
    const s = scene([floor, ['Ledge', [0, 2, -10], [12, 4, 6]]]);
    run(s, 1, idle);
    run(s, 1.6, fwd);
  });
  scenario('rungs', () => {
    const s = scene([floor, ...[0, 1, 2, 3, 4, 5, 6, 7].map((k) => ['Rung' + k, [0, 1.5 + k * 2, -6], [4, 0.4, 0.4]]), ['Top', [0, 16.2, -9], [8, 1, 6]]]);
    run(s, 1, idle);
    run(s, 4, fwd);
  });
  scenario('truss', () => {
    const s = scene([floor, ['Truss', [0, 10, -3], [2, 20, 2], 'TrussPart']]);
    run(s, 1, idle);
    run(s, 1.5, fwd);
    run(s, 1, idle);
    run(s, 1, back);
    run(s, 2, back);
  });
  scenario('trussjump', () => {
    const s = scene([floor, ['Truss', [0, 10, -3], [2, 20, 2], 'TrussPart']]);
    run(s, 1, idle);
    run(s, 1, fwd);
    run(s, 0.02, { move: [0, 0, -1], jump: true });
    run(s, 1.5, fwd);
  });
  const players = (at) => {
    const s = scene([['Floor', [0, -0.5, 0], [80, 1, 80]]]);
    const { model } = buildCharacter(s.ph.dm, 'Other', at);
    const proxy = new CharacterProxy(s.ph, model, { sensors: false, blocking: true });
    proxy.update(C.cf(at[0], at[1], at[2]));
    s.ph.world.step();
    return s;
  };
  scenario('player-block', () => {
    const s = players([0, 3, -8]);
    run(s, 0.5, idle);
    run(s, 1.5, fwd);
  });
  scenario('player-inside', () => {
    const s = players([0.4, 3, -0.3]);
    run(s, 2, fwd);
  });
  scenario('player-glance', () => {
    const s = players([1.2, 3, -8]);
    run(s, 0.5, idle);
    run(s, 2, fwd);
  });
  return out;
}
