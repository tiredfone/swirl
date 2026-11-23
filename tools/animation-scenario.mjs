const cfx = (p, r) => '<CoordinateFrame name="CFrame"><X>' + p[0] + '</X><Y>' + p[1] + '</Y><Z>' + p[2] + '</Z>' + ['R00', 'R01', 'R02', 'R10', 'R11', 'R12', 'R20', 'R21', 'R22'].map((n, i) => '<' + n + '>' + r[i] + '</' + n + '>').join('') + '</CoordinateFrame>';
const rot = (a) => [Math.cos(a), -Math.sin(a), 0, Math.sin(a), Math.cos(a), 0, 0, 0, 1];
const rotx = (a) => [1, 0, 0, 0, Math.cos(a), -Math.sin(a), 0, Math.sin(a), Math.cos(a)];

export function sequenceXml(seq) {
  let s = '<roblox version="4"><Item class="KeyframeSequence"><Properties><string name="Name">Seq</string><bool name="Loop">' + !!seq.loop + '</bool><token name="Priority">' + seq.priority + '</token></Properties>';
  for (const k of seq.keys) {
    s += '<Item class="Keyframe"><Properties><string name="Name">' + (k.name || 'Keyframe') + '</string><float name="Time">' + k.t + '</float></Properties>';
    for (const p of k.poses) {
      s += '<Item class="Pose"><Properties><string name="Name">' + p.name + '</string>' + cfx(p.pos, p.rot) + '<token name="EasingStyle">' + (p.style || 0) + '</token><token name="EasingDirection">' + (p.dir || 0) + '</token><float name="Weight">' + (p.weight == null ? 1 : p.weight) + '</float></Properties></Item>';
    }
    s += '</Item>';
  }
  return s + '</Item></roblox>';
}

export const scenario = {
  parts: ['Torso', 'Head', 'Left Arm', 'Right Arm'],
  motors: [['Neck', 'Torso', 'Head'], ['Left Shoulder', 'Torso', 'Left Arm'], ['Right Shoulder', 'Torso', 'Right Arm']],
  frames: 420,
  seqs: {
    1: { priority: 0, loop: true, keys: [
      { t: 0, poses: [{ name: 'Head', pos: [0, 0, 0], rot: rot(0) }, { name: 'Left Arm', pos: [0, 0, 0], rot: rotx(0) }] },
      { t: 0.5, poses: [{ name: 'Head', pos: [0, 0.1, 0], rot: rot(0.4), style: 3, dir: 1 }, { name: 'Left Arm', pos: [0, 0, 0], rot: rotx(1), style: 2, dir: 0 }] },
      { t: 1.2, name: 'Mid', poses: [{ name: 'Head', pos: [0, 0, 0], rot: rot(-0.3), style: 4, dir: 2 }, { name: 'Left Arm', pos: [0, 0, 0], rot: rotx(-0.5) }] },
    ] },
    2: { priority: 2, loop: false, keys: [
      { t: 0, poses: [{ name: 'Left Arm', pos: [0, 0, 0], rot: rotx(0.2) }, { name: 'Right Arm', pos: [0, 0, 0], rot: rotx(0) }] },
      { t: 0.4, name: 'Hit', poses: [{ name: 'Left Arm', pos: [0, 0.2, 0], rot: rotx(2), style: 1 }, { name: 'Right Arm', pos: [0, 0, 0], rot: rotx(-1.2) }] },
      { t: 0.9, poses: [{ name: 'Left Arm', pos: [0, 0, 0], rot: rotx(0.5) }, { name: 'Right Arm', pos: [0, 0, 0], rot: rotx(0.3), weight: 0 }] },
    ] },
    3: { priority: 1, loop: true, keys: [
      { t: 0, poses: [{ name: 'Right Arm', pos: [0, 0, 0], rot: rotx(-0.8) }, { name: 'Head', pos: [0, 0, 0], rot: rot(0.2) }] },
      { t: 0.7, poses: [{ name: 'Right Arm', pos: [0, 0, 0], rot: rotx(0.8) }, { name: 'Head', pos: [0, 0, 0], rot: rot(-0.2) }] },
    ] },
  },
  ops: [
    { f: 0, key: 'a', op: 'play', id: 1, a: 0.1, b: 1, c: 1 },
    { f: 30, key: 'b', op: 'play', id: 3, a: 0.3, b: 0.7, c: 1.5 },
    { f: 80, key: 'c', op: 'play', id: 2, a: 0.1, b: 1, c: 1 },
    { f: 120, key: 'a', op: 'speed', a: 2 },
    { f: 150, key: 'b', op: 'weight', a: 1, b: 0.4 },
    { f: 200, key: 'b', op: 'stop', a: 0.2 },
    { f: 230, key: 'c', op: 'play', id: 2, a: 0, b: 1, c: -1 },
    { f: 300, key: 'a', op: 'stop', a: 0 },
    { f: 340, key: 'd', op: 'play', id: 3, a: 0.5, b: 1, c: 1 },
    { f: 350, key: 'd', op: 'time', a: 0.3 },
  ],
};
