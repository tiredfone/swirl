use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use swirl_net::controller::{Controller, Input};
use swirl_net::mat::{self, M4};
use swirl_net::world::World;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{HtmlCanvasElement, WebGl2RenderingContext as G, WebGlProgram, WebGlUniformLocation, WebSocket};

const VS: &str = "#version 300 es
in vec3 p; in vec3 n; in vec2 uv;
uniform mat4 vp; uniform mat4 m; uniform mat4 lvp; uniform vec3 eye; uniform vec3 fwd; uniform vec3 psz; uniform int umode; uniform vec2 uvs; uniform float nbias;
out vec3 vWN; out vec3 vWV; out vec2 vUv; out vec3 vTP; out vec4 vSC; out float vDepth; flat out int vFace;
void main(){
  vec4 wp = m * vec4(p, 1.0);
  mat3 nm = transpose(inverse(mat3(m)));
  vec3 wn = normalize(nm * n);
  vWN = wn; vWV = eye - wp.xyz;
  vec2 uu = uv; vec3 tp = vec3(0.0); int face = 0;
  if (umode == 0) {
    vec3 a = abs(n);
    if (a.x > 0.5) { if (n.x > 0.0) { face = 0; uu = vec2((0.5 - p.z) * psz.z, (p.y + 0.5) * psz.y); tp.z = 0.5 * psz.x; } else { face = 1; uu = vec2((p.z + 0.5) * psz.z, (p.y + 0.5) * psz.y); tp.z = 0.5 * psz.x; } }
    else if (a.y > 0.5) { if (n.y > 0.0) { face = 2; uu = vec2((p.x + 0.5) * psz.x, (0.5 - p.z) * psz.z); tp.z = 0.5 * psz.y; } else { face = 3; uu = vec2((p.x + 0.5) * psz.x, (p.z + 0.5) * psz.z); tp.z = 0.5 * psz.y; } }
    else { if (n.z > 0.0) { face = 4; uu = vec2((p.x + 0.5) * psz.x, (p.y + 0.5) * psz.y); tp.z = 0.5 * psz.z; } else { face = 5; uu = vec2((0.5 - p.x) * psz.x, (p.y + 0.5) * psz.y); tp.z = 0.5 * psz.z; } }
    tp.xy = uu;
  } else if (umode == 1) {
    vec3 sp = p * psz; int ax = int(uv.x + 0.5);
    uu = vec2(ax == 0 ? sp.z : sp.x, ax == 1 ? sp.z : sp.y);
    tp = vec3(uu, dot(sp, normalize(n / psz)));
  } else if (umode == 3) { uu = uv; tp = vec3(uv, dot(p, n)); } else { uu = uv * uvs; }
  vUv = uu; vTP = tp; vFace = face;
  vec4 sw = vec4(wp.xyz + wn * nbias, 1.0);
  vSC = lvp * sw;
  vDepth = dot(wp.xyz - eye, fwd);
  gl_Position = vp * wp;
}";
const FS: &str = "#version 300 es
precision highp float; precision highp sampler2DShadow;
in vec3 vWN; in vec3 vWV; in vec2 vUv; in vec3 vTP; in vec4 vSC; in float vDepth; flat in int vFace;
uniform vec3 diffuse; uniform float alpha; uniform float ks; uniform float specExp; uniform float cutoff; uniform int neon; uniform int useMap; uniform int matMode; uniform vec4 matU; uniform vec4 matV;
uniform float studRow[6]; uniform int era2018; uniform int studOnly; uniform int useShadow;
uniform vec3 lamp0Dir; uniform vec3 lamp0Color; uniform vec3 lamp1Dir; uniform vec3 lamp1Color; uniform vec3 ambColor; uniform vec3 shadowTint;
uniform vec3 fogColor; uniform vec2 fogRange; uniform int fogOn; uniform vec2 shSize; uniform float refl; uniform int useEnv; uniform int useStuds;
uniform sampler2D map; uniform sampler2D matMap; uniform sampler2D studMap; uniform sampler2DShadow shadowMap; uniform samplerCube envMap;
out vec4 o;
vec4 toGamma(vec4 c) { return vec4(mix(pow(c.rgb, vec3(0.41666)) * 1.055 - vec3(0.055), c.rgb * 12.92, vec3(lessThanEqual(c.rgb, vec3(0.0031308)))), c.a); }
float softWhite1(float x) { if (x <= 0.75) return x; return 0.75 + 0.25 * (1.0 - exp(4.0 * (0.75 - x))); }
vec3 softWhite(vec3 v) { return vec3(softWhite1(v.r), softWhite1(v.g), softWhite1(v.b)); }
float ign(vec2 position) { return fract(52.9829189 * fract(dot(position, vec2(0.06711056, 0.00583715)))); }
vec2 vogel(int i, float phi) { float r = sqrt((float(i) + 0.5) / 5.0); float th = float(i) * 2.399963229728653 + phi; return vec2(cos(th), sin(th)) * r; }
float shadowMask() {
  if (useShadow == 0) return 1.0;
  vec3 c = vSC.xyz / vSC.w; c = c * 0.5 + 0.5; c.z -= 0.0004;
  if (c.x < 0.0 || c.x > 1.0 || c.y < 0.0 || c.y > 1.0 || c.z > 1.0) return 1.0;
  vec2 ts = 1.0 / shSize; float phi = ign(gl_FragCoord.xy) * 6.283185307179586; float s = 0.0;
  for (int i = 0; i < 5; i++) s += texture(shadowMap, vec3(c.xy + vogel(i, phi) * ts, c.z));
  return s * 0.2;
}
void main() {
  vec3 N = normalize(vWN); if (!gl_FrontFacing) N = -N;
  vec3 V = normalize(vWV);
  vec4 base = vec4(diffuse, alpha);
  if (useMap != 0) { vec4 t = toGamma(texture(map, vUv)); base *= t; if (base.a < cutoff) discard; }
  float sm = shadowMask();
  if (studOnly != 0) {
    vec2 so = vec2(vUv.x * 0.5, vUv.y * 0.25); vec2 sf = fract(so); float row = studRow[0];
    vec4 sv = toGamma(textureGrad(studMap, vec2(sf.x, (4.0 - row + sf.y) / 5.0), dFdx(so) * vec2(1.0, 0.2), dFdy(so) * vec2(1.0, 0.2)));
    if (sv.a < 0.004) discard;
    o = vec4(sv.rgb * mix(shadowTint, vec3(1.0), sm * smoothstep(0.0, 0.25, dot(N, lamp0Dir))), sv.a);
    if (fogOn != 0) o.rgb = mix(o.rgb, fogColor, smoothstep(fogRange.x, fogRange.y, vDepth));
    return;
  }
  vec3 lit;
  float ndl0 = dot(N, lamp0Dir);
  if (neon != 0) { lit = base.rgb; }
  else {
    vec3 H = normalize(V + lamp0Dir);
    float spec = ndl0 > 0.0 ? pow(max(dot(N, H), 0.0), specExp) : 0.0;
    lit = softWhite(max(base.rgb * (max(ndl0, 0.0) * lamp0Color + max(dot(N, lamp1Dir), 0.0) * lamp1Color + ambColor), vec3(0.0)));
    if (matMode >= 0) {
      vec2 mt = vec2(dot(matU, vec4(vTP, 1.0)), dot(matV, vec4(vTP, 1.0)));
      vec4 m = toGamma(texture(matMap, mt));
      if (matMode == 1) lit = mix(lit, m.rgb, m.a);
      else if (matMode == 2) lit = lit * clamp(m.rgb * 1.45, 0.0, 1.6);
      else lit = lit + m.rgb - 0.5;
    }
    if (useStuds != 0 && studRow[vFace] >= 0.0) {
      float row = studRow[vFace];
      if (era2018 != 0) {
        vec2 su = vec2(vUv.x * 0.5, vUv.y * 0.125); vec2 f = fract(su);
        float sg = toGamma(textureGrad(studMap, vec2(f.x, (3.0 - row + f.y) / 4.0), dFdx(su) * vec2(1.0, 0.25), dFdy(su) * vec2(1.0, 0.25))).r;
        lit = lit * (sg * 2.0);
      } else {
        vec2 su = vec2(vUv.x * 0.5, vUv.y * 0.25); vec2 f = fract(su);
        vec4 s = toGamma(textureGrad(studMap, vec2(f.x, (4.0 - row + f.y) / 5.0), dFdx(su) * vec2(1.0, 0.2), dFdy(su) * vec2(1.0, 0.2)));
        lit = mix(lit, s.rgb, s.a);
      }
    }
    lit += ks * spec * lamp0Color;
    if (useEnv != 0) { vec3 r = reflect(-V, N); vec3 env = texture(envMap, vec3(-r.x, r.y, r.z)).rgb; lit = mix(lit, env, refl); }
    lit = clamp(softWhite(max(lit, vec3(0.0))), 0.0, 1.0);
    if (era2018 != 0) lit *= mix(shadowTint, vec3(1.0), mix(1.0, sm, smoothstep(0.0, 0.25, ndl0)));
    else lit *= mix(shadowTint, vec3(1.0), sm * smoothstep(0.0, 0.25, ndl0));
  }
  o = vec4(lit, base.a);
  if (fogOn != 0) o.rgb = mix(o.rgb, fogColor, smoothstep(fogRange.x, fogRange.y, vDepth));
}";
const COMP_VS: &str = "#version 300 es
in vec3 p; in vec3 n; in vec2 uv;
uniform mat4 vp; out vec2 vuv;
void main(){ vuv = uv; gl_Position = vp * vec4(p.xy, 0.0, 1.0); }";
const COMP_FS: &str = "#version 300 es
precision highp float;
in vec2 vuv; uniform vec4 col; uniform int useTex; uniform sampler2D tex; out vec4 o;
void main(){ o = useTex != 0 ? texture(tex, vuv) : col; }";
const DEPTH_VS: &str = "#version 300 es
in vec3 p; in vec3 n; in vec2 uv;
uniform mat4 lvp; uniform mat4 m;
void main(){ gl_Position = lvp * m * vec4(p, 1.0); }";
const DEPTH_FS: &str = "#version 300 es
precision mediump float;
out vec4 o;
void main(){ o = vec4(1.0); }";

const SKY_VS: &str = "#version 300 es
out vec2 q;
void main(){ vec2 p = vec2(float((gl_VertexID << 1) & 2), float(gl_VertexID & 2)); q = p * 2.0 - 1.0; gl_Position = vec4(q, 0.9999, 1.0); }";
const SKY_FS: &str = "#version 300 es
precision highp float;
in vec2 q; out vec4 o;
uniform vec3 dir; uniform vec3 right; uniform vec3 up; uniform vec2 tanv;
uniform int dome; uniform vec3 sunDir; uniform vec3 ctop; uniform vec3 chor; uniform vec3 cbot; uniform float sunVis;
uniform samplerCube cube;
void main(){
  vec3 r = dir + q.x * tanv.x * right + q.y * tanv.y * up;
  if (dome != 0) {
    vec3 d = normalize(r); float h = d.y;
    vec3 c = h > 0.0 ? mix(chor, ctop, pow(clamp(h, 0.0, 1.0), 0.55)) : mix(chor, cbot, clamp(-h * 3.0, 0.0, 1.0));
    float sd = max(dot(d, normalize(sunDir)), 0.0);
    c += vec3(1.0, 0.95, 0.8) * (pow(sd, 900.0) * 1.6 + pow(sd, 12.0) * 0.18) * sunVis;
    o = vec4(c, 1.0);
    return;
  }
  o = vec4(texture(cube, vec3(-r.x, r.y, r.z)).rgb, 1.0);
}";


struct Prog {
    prog: WebGlProgram,
    locs: RefCell<HashMap<&'static str, Option<WebGlUniformLocation>>>,
}

impl Prog {
    fn l(&self, gl: &G, name: &'static str) -> Option<WebGlUniformLocation> {
        let mut m = self.locs.borrow_mut();
        m.entry(name).or_insert_with(|| gl.get_uniform_location(&self.prog, name)).clone()
    }
}

struct MatDef {
    ks: f32,
    exp: f32,
    tex: Option<String>,
    u: [f32; 4],
    v: [f32; 4],
    blend: bool,
    mul: bool,
    neon: bool,
}

enum Geo {
    Terrain(String, usize),
    Shape(usize),
    Mesh(String),
    Plane,
}

struct Item {
    geo: Geo,
    m: M4,
    psz: [f32; 3],
    mode: i32,
    uvs: [f32; 2],
    color: [f32; 3],
    alpha: f32,
    ks: f32,
    exp: f32,
    cutoff: f32,
    neon: bool,
    map: Option<String>,
    map_repeat: bool,
    mat: Option<MatDef>,
    studs: Option<[f32; 6]>,
    stud_only: bool,
    cast: bool,
    receive: bool,
    cull: bool,
    offset: bool,
    dist: f32,
}

struct Shadow {
    fbo: Option<web_sys::WebGlFramebuffer>,
    tex: Option<web_sys::WebGlTexture>,
    size: i32,
}

struct State {
    pp: Prog,
    dp: Prog,
    sp: Prog,
    shadow: Shadow,
    samp_repeat: Option<web_sys::WebGlSampler>,
    samp_clamp: Option<web_sys::WebGlSampler>,
    shapes: Vec<Mesh>,
    plane: Mesh,
    sky_vao: Option<web_sys::WebGlVertexArrayObject>,
    world: World,
    gl: G,
    canvas: HtmlCanvasElement,
    yaw: f32,
    pitch: f32,
    dist: f32,
    target: [f32; 3],
    fitted: bool,
    drag: Option<(f32, f32)>,
    status: String,
    ctl: Option<Controller>,
    keys: HashSet<String>,
    last_t: f64,
    send_acc: f64,
    ws: Option<WebSocket>,
    follow: bool,
    failed: HashSet<String>,
    focus: Option<[f32; 3]>,
    meshes: HashMap<String, Mesh>,
    textures: HashMap<String, Option<web_sys::WebGlTexture>>,
    requested: HashSet<String>,
    remotes: Vec<String>,
    tap: Vec<String>,
    eye: [f32; 3],
    view_fov: f32,
    driven: bool,
    has_cam: bool,
    cam_cf: [f32; 12],
    cam_fov: f32,
    queue: Vec<(String, String)>,
    aniso: f32,
    max_tex: i32,
    dummy2d: Option<web_sys::WebGlTexture>,
    dummy_cube: Option<web_sys::WebGlTexture>,
    sky_cube: Option<web_sys::WebGlTexture>,
    terrain: HashMap<String, Vec<(Mesh, [f32; 3], String, bool, f32)>>,
    cp: Prog,
    atlas_fbo: Option<web_sys::WebGlFramebuffer>,
    atlases: HashMap<u32, String>,
    atlases_r15: HashMap<(u32, u8), String>,
}

struct Mesh {
    vao: Option<web_sys::WebGlVertexArrayObject>,
    count: i32,
    lo: [f32; 3],
    hi: [f32; 3],
}

#[wasm_bindgen]
pub struct Client {
    st: Rc<RefCell<State>>,
    _ws: Option<WebSocket>,
}

fn shader(gl: &G, kind: u32, src: &str) -> web_sys::WebGlShader {
    let s = gl.create_shader(kind).unwrap();
    gl.shader_source(&s, src);
    gl.compile_shader(&s);
    s
}

#[rustfmt::skip]
fn cube() -> Vec<f32> {
    let faces: [([f32; 3], [[f32; 3]; 4]); 6] = [
        ([1.0, 0.0, 0.0], [[0.5, -0.5, -0.5], [0.5, 0.5, -0.5], [0.5, 0.5, 0.5], [0.5, -0.5, 0.5]]),
        ([-1.0, 0.0, 0.0], [[-0.5, -0.5, 0.5], [-0.5, 0.5, 0.5], [-0.5, 0.5, -0.5], [-0.5, -0.5, -0.5]]),
        ([0.0, 1.0, 0.0], [[-0.5, 0.5, -0.5], [-0.5, 0.5, 0.5], [0.5, 0.5, 0.5], [0.5, 0.5, -0.5]]),
        ([0.0, -1.0, 0.0], [[-0.5, -0.5, 0.5], [-0.5, -0.5, -0.5], [0.5, -0.5, -0.5], [0.5, -0.5, 0.5]]),
        ([0.0, 0.0, 1.0], [[0.5, -0.5, 0.5], [0.5, 0.5, 0.5], [-0.5, 0.5, 0.5], [-0.5, -0.5, 0.5]]),
        ([0.0, 0.0, -1.0], [[-0.5, -0.5, -0.5], [-0.5, 0.5, -0.5], [0.5, 0.5, -0.5], [0.5, -0.5, -0.5]]),
    ];
    let mut v = vec![];
    let uvs = [[0.0f32, 1.0], [0.0, 0.0], [1.0, 0.0], [1.0, 1.0]];
    for (n, q) in faces.iter() {
        for i in [0usize, 1, 2, 0, 2, 3] {
            v.extend_from_slice(&q[i]);
            v.extend_from_slice(n);
            v.extend_from_slice(&uvs[i]);
        }
    }
    v
}

fn sphere_data() -> Vec<f32> {
    let (lon, lat) = (24usize, 16usize);
    let pt = |i: usize, j: usize| -> [f32; 3] {
        let th = i as f32 / lon as f32 * std::f32::consts::TAU;
        let ph = j as f32 / lat as f32 * std::f32::consts::PI;
        [ph.sin() * th.cos(), ph.cos(), ph.sin() * th.sin()]
    };
    let mut v = vec![];
    for i in 0..lon {
        for j in 0..lat {
            let (a, b, c, d) = (pt(i, j), pt(i + 1, j), pt(i + 1, j + 1), pt(i, j + 1));
            for q in [a, c, b, a, d, c] {
                v.extend_from_slice(&[q[0] * 0.5, q[1] * 0.5, q[2] * 0.5, q[0], q[1], q[2], 0.0, 0.0]);
            }
        }
    }
    v
}

fn cylinder_data() -> Vec<f32> {
    let n = 24usize;
    let mut v = vec![];
    for i in 0..n {
        let a0 = i as f32 / n as f32 * std::f32::consts::TAU;
        let a1 = (i + 1) as f32 / n as f32 * std::f32::consts::TAU;
        let (y0, z0, y1, z1) = (a0.cos(), a0.sin(), a1.cos(), a1.sin());
        let p = |x: f32, y: f32, z: f32, n: [f32; 3]| [x, y * 0.5, z * 0.5, n[0], n[1], n[2], 0.0, 0.0];
        let quad = [p(-0.5, y0, z0, [0.0, y0, z0]), p(0.5, y0, z0, [0.0, y0, z0]), p(0.5, y1, z1, [0.0, y1, z1]), p(-0.5, y1, z1, [0.0, y1, z1])];
        for k in [0usize, 2, 1, 0, 3, 2] {
            v.extend_from_slice(&quad[k]);
        }
        for (x, nx) in [(0.5f32, 1.0f32), (-0.5, -1.0)] {
            let c = [x, 0.0, 0.0, nx, 0.0, 0.0, 0.0, 0.0];
            let (a, b) = (p(x, y0, z0, [nx, 0.0, 0.0]), p(x, y1, z1, [nx, 0.0, 0.0]));
            if nx > 0.0 {
                v.extend_from_slice(&c);
                v.extend_from_slice(&b);
                v.extend_from_slice(&a);
            } else {
                v.extend_from_slice(&c);
                v.extend_from_slice(&a);
                v.extend_from_slice(&b);
            }
        }
    }
    v
}

fn cylinder_y_data() -> Vec<f32> {
    let src = cylinder_data();
    let mut out = Vec::with_capacity(src.len());
    for tri in src.chunks(24) {
        let mut v: Vec<[f32; 8]> = tri.chunks(8).map(|c| [c[1], c[0], c[2], c[4], c[3], c[5], c[6], c[7]]).collect();
        v.swap(1, 2);
        for q in v {
            out.extend_from_slice(&q);
        }
    }
    out
}

fn wedge_data() -> Vec<f32> {
    let (a, b, c, d, e, f) = ([-0.5f32, -0.5, -0.5], [0.5, -0.5, -0.5], [0.5, -0.5, 0.5], [-0.5, -0.5, 0.5], [-0.5, 0.5, 0.5], [0.5, 0.5, 0.5]);
    let faces: [Vec<[f32; 3]>; 5] = [vec![a, b, c, d], vec![d, c, f, e], vec![a, e, f, b], vec![a, d, e], vec![b, f, c]];
    let mut v = vec![];
    for q in faces.iter() {
        let e1 = [q[1][0] - q[0][0], q[1][1] - q[0][1], q[1][2] - q[0][2]];
        let e2 = [q[2][0] - q[0][0], q[2][1] - q[0][1], q[2][2] - q[0][2]];
        let mut n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
        let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-9);
        n = [n[0] / l, n[1] / l, n[2] / l];
        let mid = q.iter().fold([0.0f32; 3], |s, p| [s[0] + p[0], s[1] + p[1], s[2] + p[2]]);
        let flip = n[0] * mid[0] + n[1] * mid[1] + n[2] * mid[2] < 0.0;
        let mut pts = q.clone();
        if flip {
            pts.reverse();
            n = [-n[0], -n[1], -n[2]];
        }
        for i in 1..pts.len() - 1 {
            for p in [pts[0], pts[i], pts[i + 1]] {
                v.extend_from_slice(&[p[0], p[1], p[2], n[0], n[1], n[2], 0.0, 0.0]);
            }
        }
    }
    v
}

fn make_mesh(gl: &G, data: &[f32]) -> Mesh {
    let vao = gl.create_vertex_array();
    gl.bind_vertex_array(vao.as_ref());
    let buf = gl.create_buffer();
    gl.bind_buffer(G::ARRAY_BUFFER, buf.as_ref());
    unsafe {
        let view = js_sys_f32(data);
        gl.buffer_data_with_array_buffer_view(G::ARRAY_BUFFER, &view, G::STATIC_DRAW);
    }
    for (loc, size, off) in [(0u32, 3, 0), (1, 3, 12), (2, 2, 24)] {
        gl.enable_vertex_attrib_array(loc);
        gl.vertex_attrib_pointer_with_i32(loc, size, G::FLOAT, false, 32, off);
    }
    let mut lo = [f32::INFINITY; 3];
    let mut hi = [f32::NEG_INFINITY; 3];
    for v in data.chunks(8) {
        for k in 0..3 {
            lo[k] = lo[k].min(v[k]);
            hi[k] = hi[k].max(v[k]);
        }
    }
    Mesh { vao, count: (data.len() / 8) as i32, lo, hi }
}

fn with_axis(mut data: Vec<f32>) -> Vec<f32> {
    for tri in data.chunks_mut(24) {
        let a = [tri[0], tri[1], tri[2]];
        let b = [tri[8], tri[9], tri[10]];
        let c = [tri[16], tri[17], tri[18]];
        let e1 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let e2 = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let n = cross3(e1, e2);
        let (ax, ay, az) = (n[0].abs(), n[1].abs(), n[2].abs());
        let axis = if ax >= ay && ax >= az { 0.0 } else if ay >= az { 1.0 } else { 2.0 };
        for k in 0..3 {
            tri[k * 8 + 6] = axis;
            tri[k * 8 + 7] = 0.0;
        }
    }
    data
}

fn corner_wedge_data() -> Vec<f32> {
    let (a, b, c, d, t) = ([-0.5f32, -0.5, -0.5], [0.5, -0.5, -0.5], [0.5, -0.5, 0.5], [-0.5, -0.5, 0.5], [0.5, 0.5, -0.5]);
    let tris: [[[f32; 3]; 3]; 6] = [[a, d, c], [a, c, b], [b, c, t], [a, b, t], [d, t, c], [a, t, d]];
    let mid = {
        let all = [a, b, c, d, t];
        let mut m = [0.0f32; 3];
        for p in all.iter() {
            for k in 0..3 {
                m[k] += p[k] / 5.0;
            }
        }
        m
    };
    let mut v = vec![];
    for q in tris.iter() {
        let e1 = [q[1][0] - q[0][0], q[1][1] - q[0][1], q[1][2] - q[0][2]];
        let e2 = [q[2][0] - q[0][0], q[2][1] - q[0][1], q[2][2] - q[0][2]];
        let mut n = cross3(e1, e2);
        let cx = [q[0][0] + q[1][0] + q[2][0] - 3.0 * mid[0], q[0][1] + q[1][1] + q[2][1] - 3.0 * mid[1], q[0][2] + q[1][2] + q[2][2] - 3.0 * mid[2]];
        let mut pts = [q[0], q[1], q[2]];
        if n[0] * cx[0] + n[1] * cx[1] + n[2] * cx[2] < 0.0 {
            pts.swap(1, 2);
            n = [-n[0], -n[1], -n[2]];
        }
        let n = norm3(n);
        for p in pts.iter() {
            v.extend_from_slice(&[p[0], p[1], p[2], n[0], n[1], n[2], 0.0, 0.0]);
        }
    }
    v
}

fn plane_data() -> Vec<f32> {
    let n = [0.0f32, 0.0, 1.0];
    let q = [([-0.5f32, -0.5, 0.0], [0.0f32, 0.0]), ([0.5, -0.5, 0.0], [1.0, 0.0]), ([0.5, 0.5, 0.0], [1.0, 1.0]), ([-0.5, 0.5, 0.0], [0.0, 1.0])];
    let mut v = vec![];
    for i in [0usize, 1, 2, 0, 2, 3] {
        v.extend_from_slice(&[q[i].0[0], q[i].0[1], q[i].0[2], n[0], n[1], n[2], q[i].1[0], q[i].1[1]]);
    }
    v
}

#[wasm_bindgen]
impl Client {
    #[wasm_bindgen(constructor)]
    pub fn new(canvas: HtmlCanvasElement) -> Result<Client, JsValue> {
        let opts = web_sys::js_sys::Object::new();
        let _ = web_sys::js_sys::Reflect::set(&opts, &"alpha".into(), &false.into());
        let _ = web_sys::js_sys::Reflect::set(&opts, &"antialias".into(), &false.into());
        let _ = web_sys::js_sys::Reflect::set(&opts, &"powerPreference".into(), &"high-performance".into());
        let gl: G = canvas.get_context_with_context_options("webgl2", &opts)?.ok_or("WebGL2 is not available")?.dyn_into()?;
        let build = |vs: &str, fs: &str| -> WebGlProgram {
            let prog = gl.create_program().unwrap();
            gl.attach_shader(&prog, &shader(&gl, G::VERTEX_SHADER, vs));
            gl.attach_shader(&prog, &shader(&gl, G::FRAGMENT_SHADER, fs));
            gl.bind_attrib_location(&prog, 0, "p");
            gl.bind_attrib_location(&prog, 1, "n");
            gl.bind_attrib_location(&prog, 2, "uv");
            gl.link_program(&prog);
            prog
        };
        let pp = Prog { prog: build(VS, FS), locs: RefCell::new(HashMap::new()) };
        let dp = Prog { prog: build(DEPTH_VS, DEPTH_FS), locs: RefCell::new(HashMap::new()) };
        let sp = Prog { prog: build(SKY_VS, SKY_FS), locs: RefCell::new(HashMap::new()) };
        let cp = Prog { prog: build(COMP_VS, COMP_FS), locs: RefCell::new(HashMap::new()) };
        let atlas_fbo = gl.create_framebuffer();
        let sky_vao = gl.create_vertex_array();
        let mut shapes = vec![];
        shapes.push(make_mesh(&gl, &with_axis(sphere_data())));
        shapes.push(make_mesh(&gl, &cube()));
        shapes.push(make_mesh(&gl, &with_axis(cylinder_data())));
        shapes.push(make_mesh(&gl, &with_axis(wedge_data())));
        shapes.push(make_mesh(&gl, &with_axis(cylinder_y_data())));
        shapes.push(make_mesh(&gl, &with_axis(corner_wedge_data())));
        let plane = make_mesh(&gl, &plane_data());
        gl.enable(G::DEPTH_TEST);
        gl.enable(G::CULL_FACE);
        gl.enable(G::BLEND);
        gl.blend_func_separate(G::SRC_ALPHA, G::ONE_MINUS_SRC_ALPHA, G::ONE, G::ONE_MINUS_SRC_ALPHA);
        let aniso_ext = gl.get_extension("EXT_texture_filter_anisotropic").ok().flatten();
        let aniso = if aniso_ext.is_some() { 4.0 } else { 1.0 };
        let mk_sampler = |wrap: u32| {
            let sm = gl.create_sampler();
            if let Some(sm) = sm.as_ref() {
                gl.sampler_parameteri(sm, G::TEXTURE_MIN_FILTER, G::LINEAR_MIPMAP_LINEAR as i32);
                gl.sampler_parameteri(sm, G::TEXTURE_MAG_FILTER, G::LINEAR as i32);
                gl.sampler_parameteri(sm, G::TEXTURE_WRAP_S, wrap as i32);
                gl.sampler_parameteri(sm, G::TEXTURE_WRAP_T, wrap as i32);
                if aniso > 1.0 {
                    gl.sampler_parameterf(sm, 0x84FE, aniso);
                }
            }
            sm
        };
        let samp_repeat = mk_sampler(G::REPEAT);
        let samp_clamp = mk_sampler(G::CLAMP_TO_EDGE);
        let max_tex = gl.get_parameter(G::MAX_TEXTURE_SIZE).ok().and_then(|v| v.as_f64()).unwrap_or(2048.0) as i32;
        let ssize = max_tex.min(4096);
        let stex = gl.create_texture();
        gl.bind_texture(G::TEXTURE_2D, stex.as_ref());
        gl.tex_storage_2d(G::TEXTURE_2D, 1, G::DEPTH_COMPONENT24, ssize, ssize);
        gl.tex_parameteri(G::TEXTURE_2D, G::TEXTURE_MIN_FILTER, G::LINEAR as i32);
        gl.tex_parameteri(G::TEXTURE_2D, G::TEXTURE_MAG_FILTER, G::LINEAR as i32);
        gl.tex_parameteri(G::TEXTURE_2D, G::TEXTURE_WRAP_S, G::CLAMP_TO_EDGE as i32);
        gl.tex_parameteri(G::TEXTURE_2D, G::TEXTURE_WRAP_T, G::CLAMP_TO_EDGE as i32);
        gl.tex_parameteri(G::TEXTURE_2D, G::TEXTURE_COMPARE_MODE, G::COMPARE_REF_TO_TEXTURE as i32);
        gl.tex_parameteri(G::TEXTURE_2D, G::TEXTURE_COMPARE_FUNC, G::LEQUAL as i32);
        let fbo = gl.create_framebuffer();
        gl.bind_framebuffer(G::FRAMEBUFFER, fbo.as_ref());
        gl.framebuffer_texture_2d(G::FRAMEBUFFER, G::DEPTH_ATTACHMENT, G::TEXTURE_2D, stex.as_ref(), 0);
        let none = web_sys::js_sys::Array::of1(&JsValue::from(G::NONE));
        gl.draw_buffers(&none);
        gl.read_buffer(G::NONE);
        gl.bind_framebuffer(G::FRAMEBUFFER, None);
        let white = [255u8, 255, 255, 255];
        let dummy2d = gl.create_texture();
        gl.bind_texture(G::TEXTURE_2D, dummy2d.as_ref());
        let _ = gl.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(G::TEXTURE_2D, 0, G::RGBA as i32, 1, 1, 0, G::RGBA, G::UNSIGNED_BYTE, Some(&white));
        gl.tex_parameteri(G::TEXTURE_2D, G::TEXTURE_MIN_FILTER, G::NEAREST as i32);
        gl.tex_parameteri(G::TEXTURE_2D, G::TEXTURE_MAG_FILTER, G::NEAREST as i32);
        let dummy_cube = gl.create_texture();
        gl.bind_texture(G::TEXTURE_CUBE_MAP, dummy_cube.as_ref());
        for f in 0..6u32 {
            let _ = gl.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(G::TEXTURE_CUBE_MAP_POSITIVE_X + f, 0, G::RGBA as i32, 1, 1, 0, G::RGBA, G::UNSIGNED_BYTE, Some(&white));
        }
        gl.tex_parameteri(G::TEXTURE_CUBE_MAP, G::TEXTURE_MIN_FILTER, G::NEAREST as i32);
        gl.tex_parameteri(G::TEXTURE_CUBE_MAP, G::TEXTURE_MAG_FILTER, G::NEAREST as i32);
        let st = Rc::new(RefCell::new(State {
            pp,
            dp,
            sp,
            shadow: Shadow { fbo, tex: stex, size: ssize },
            samp_repeat,
            samp_clamp,
            shapes,
            plane,
            sky_vao,
            world: World::new(),
            gl,
            canvas: canvas.clone(),
            yaw: 2.4,
            pitch: 0.5,
            dist: 60.0,
            target: [0.0, 5.0, 0.0],
            fitted: false,
            drag: None,
            status: "Starting".into(),
            ctl: None,
            keys: HashSet::new(),
            last_t: 0.0,
            send_acc: 0.0,
            ws: None,
            follow: false,
            failed: HashSet::new(),
            focus: None,
            meshes: HashMap::new(),
            textures: HashMap::new(),
            requested: HashSet::new(),
            remotes: vec![],
            tap: vec![],
            eye: [0.0, 0.0, 0.0],
            view_fov: 1.0,
            driven: false,
            has_cam: false,
            cam_cf: mat::CF_ID,
            cam_fov: 1.0,
            queue: vec![],
            aniso,
            max_tex,
            dummy2d,
            dummy_cube,
            sky_cube: None,
            terrain: HashMap::new(),
            cp,
            atlas_fbo,
            atlases: HashMap::new(),
            atlases_r15: HashMap::new(),
        }));
        let s2 = st.clone();
        let down = Closure::<dyn FnMut(web_sys::MouseEvent)>::new(move |e: web_sys::MouseEvent| {
            s2.borrow_mut().drag = Some((e.client_x() as f32, e.client_y() as f32));
        });
        canvas.add_event_listener_with_callback("mousedown", down.as_ref().unchecked_ref())?;
        down.forget();
        let s3 = st.clone();
        let mv = Closure::<dyn FnMut(web_sys::MouseEvent)>::new(move |e: web_sys::MouseEvent| {
            let mut s = s3.borrow_mut();
            if let Some((x, y)) = s.drag {
                let (nx, ny) = (e.client_x() as f32, e.client_y() as f32);
                s.yaw -= (nx - x) * 0.008;
                s.pitch = (s.pitch + (ny - y) * 0.008).clamp(-1.5, 1.5);
                s.drag = Some((nx, ny));
            }
        });
        canvas.add_event_listener_with_callback("mousemove", mv.as_ref().unchecked_ref())?;
        mv.forget();
        let s4 = st.clone();
        let up = Closure::<dyn FnMut(web_sys::MouseEvent)>::new(move |_e: web_sys::MouseEvent| {
            s4.borrow_mut().drag = None;
        });
        canvas.add_event_listener_with_callback("mouseup", up.as_ref().unchecked_ref())?;
        up.forget();
        let s5 = st.clone();
        let wheel = Closure::<dyn FnMut(web_sys::WheelEvent)>::new(move |e: web_sys::WheelEvent| {
            let mut s = s5.borrow_mut();
            s.dist = (s.dist * (1.0 + (e.delta_y() as f32).signum() * 0.1)).clamp(3.0, 2000.0);
            e.prevent_default();
        });
        canvas.add_event_listener_with_callback("wheel", wheel.as_ref().unchecked_ref())?;
        wheel.forget();
        let win = web_sys::window().ok_or("no window")?;
        let sk = st.clone();
        let kd = Closure::<dyn FnMut(web_sys::KeyboardEvent)>::new(move |e: web_sys::KeyboardEvent| {
            let k = e.key().to_lowercase();
            if k == " " || k.starts_with("arrow") {
                e.prevent_default();
            }
            sk.borrow_mut().keys.insert(k);
        });
        win.add_event_listener_with_callback("keydown", kd.as_ref().unchecked_ref())?;
        kd.forget();
        let sk2 = st.clone();
        let ku = Closure::<dyn FnMut(web_sys::KeyboardEvent)>::new(move |e: web_sys::KeyboardEvent| {
            sk2.borrow_mut().keys.remove(&e.key().to_lowercase());
        });
        win.add_event_listener_with_callback("keyup", ku.as_ref().unchecked_ref())?;
        ku.forget();
        Ok(Client { st, _ws: None })
    }

    pub fn connect(&mut self, url: &str) -> Result<(), JsValue> {
        let ws = WebSocket::new(url)?;
        let s = self.st.clone();
        let on = Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |e: web_sys::MessageEvent| {
            if let Some(t) = e.data().as_string() {
                let mut st = s.borrow_mut();
                if st.tap.len() < 6000 {
                    st.tap.push(t.clone());
                }
                if let Some(kind) = st.world.apply(&t) {
                    if kind == "init" {
                        st.fitted = false;
                        st.status = "Loaded".into();
                    }
                }
            }
        });
        ws.set_onmessage(Some(on.as_ref().unchecked_ref()));
        on.forget();
        let s = self.st.clone();
        let cl = Closure::<dyn FnMut(web_sys::CloseEvent)>::new(move |e: web_sys::CloseEvent| {
            s.borrow_mut().status = format!("Disconnected ({})", e.code());
        });
        ws.set_onclose(Some(cl.as_ref().unchecked_ref()));
        cl.forget();
        self.st.borrow_mut().status = "Connecting".into();
        self.st.borrow_mut().ws = Some(ws.clone());
        self._ws = Some(ws);
        Ok(())
    }

    pub fn take_requests(&self) -> String {
        let mut st = self.st.borrow_mut();
        let q: Vec<(String, String)> = st.queue.drain(..).collect();
        let items: Vec<String> = q.iter().map(|(k, id)| format!("{{\"k\":\"{}\",\"id\":\"{}\"}}", k, id)).collect();
        format!("[{}]", items.join(","))
    }

    pub fn provide_mesh(&self, id: &str, bytes: &[u8]) {
        let mut st = self.st.borrow_mut();
        match swirl_net::meshdata::parse(bytes) {
            Some(data) => {
                let m = make_mesh(&st.gl, &data);
                if matches!(id, "c:fonts/torso.mesh" | "c:fonts/leftarm.mesh" | "c:fonts/rightarm.mesh" | "c:fonts/leftleg.mesh" | "c:fonts/rightleg.mesh") {
                    let mut flat = data.clone();
                    let mut acc: HashMap<[i32; 3], [f32; 3]> = HashMap::new();
                    let q = |v: &[f32]| [(v[0] * 1000.0).round() as i32, (v[1] * 1000.0).round() as i32, (v[2] * 1000.0).round() as i32];
                    for tri in flat.chunks(24) {
                        let a = [tri[0], tri[1], tri[2]];
                        let b = [tri[8], tri[9], tri[10]];
                        let c = [tri[16], tri[17], tri[18]];
                        let n = cross3([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [c[0] - a[0], c[1] - a[1], c[2] - a[2]]);
                        for k in 0..3 {
                            let e = acc.entry(q(&tri[k * 8..])).or_insert([0.0; 3]);
                            e[0] += n[0];
                            e[1] += n[1];
                            e[2] += n[2];
                        }
                    }
                    for v in flat.chunks_mut(8) {
                        if let Some(n) = acc.get(&q(v)) {
                            let n = norm3(*n);
                            v[3] = n[0];
                            v[4] = n[1];
                            v[5] = n[2];
                        }
                    }
                    let fm = make_mesh(&st.gl, &flat);
                    st.meshes.insert(format!("{}#flat", id), fm);
                }
                if id == "c:fonts/head.mesh" {
                    let mut face = data.clone();
                    let (lo, hi) = (m.lo, m.hi);
                    for v in face.chunks_mut(8) {
                        if v[2] > (lo[2] + hi[2]) / 2.0 + (hi[2] - lo[2]) * 0.15 {
                            v[6] = -4.0;
                            v[7] = -4.0;
                        } else {
                            v[6] = 1.0 - (v[0] - lo[0]) / (hi[0] - lo[0]);
                            v[7] = (v[1] - lo[1]) / (hi[1] - lo[1]);
                        }
                    }
                    let fm = make_mesh(&st.gl, &face);
                    st.meshes.insert("c:fonts/head.mesh#face".to_string(), fm);
                }
                st.meshes.insert(id.to_string(), m);
            }
            None => {
                st.failed.insert(id.to_string());
            }
        }
    }

    pub fn provide_texture(&self, id: &str, img: &web_sys::ImageBitmap) {
        let mut st = self.st.borrow_mut();
        let gl = st.gl.clone();
        let t = gl.create_texture();
        gl.bind_texture(G::TEXTURE_2D, t.as_ref());
        let _ = gl.tex_image_2d_with_u32_and_u32_and_image_bitmap(G::TEXTURE_2D, 0, G::SRGB8_ALPHA8 as i32, G::RGBA, G::UNSIGNED_BYTE, img);
        gl.generate_mipmap(G::TEXTURE_2D);
        st.textures.insert(id.to_string(), t);
    }

    pub fn provide_failed(&self, kind: &str, id: &str) {
        let mut st = self.st.borrow_mut();
        if kind == "tex" {
            st.textures.insert(id.to_string(), None);
        } else {
            st.failed.insert(id.to_string());
        }
    }

    pub fn set_driven(&self, on: bool) {
        self.st.borrow_mut().driven = on;
        self.st.borrow_mut().world.driven = on;
    }

    pub fn set_camera(&self, cf: &[f32], fov_deg: f32) {
        let mut st = self.st.borrow_mut();
        if cf.len() >= 12 {
            st.cam_cf.copy_from_slice(&cf[..12]);
            st.cam_fov = (fov_deg.clamp(1.0, 120.0)).to_radians();
            st.has_cam = true;
        }
    }

    pub fn set_cfs(&self, data: &[f64]) {
        let mut st = self.st.borrow_mut();
        for c in data.chunks_exact(13) {
            let mut cf = [0.0f32; 12];
            for k in 0..12 {
                cf[k] = c[1 + k] as f32;
            }
            st.world.set_cf(c[0] as u32, &cf);
        }
    }

    pub fn apply_local(&self, text: &str) {
        self.st.borrow_mut().world.apply_local(text);
    }

    pub fn set_local_alpha(&self, id: u32, t: f32) {
        let mut st = self.st.borrow_mut();
        if t.is_nan() {
            st.world.local_t.remove(&id);
        } else {
            st.world.local_t.insert(id, t);
        }
    }

    pub fn clear_focus(&self) {
        self.st.borrow_mut().focus = None;
    }

    pub fn focus(&self, x: f32, y: f32, z: f32, dist: f32) {
        let mut st = self.st.borrow_mut();
        st.focus = Some([x, y, z]);
        st.target = [x, y, z];
        st.dist = dist;
        st.fitted = true;
    }

    pub fn debug(&self) -> String {
        let st = self.st.borrow();
        format!("{{\"target\":[{:.1},{:.1},{:.1}],\"dist\":{:.1},\"yaw\":{:.2},\"pitch\":{:.2},\"focus\":{},\"meshes\":{},\"failed\":{},\"requested\":{}}}", st.target[0], st.target[1], st.target[2], st.dist, st.yaw, st.pitch, st.focus.is_some(), st.meshes.len(), st.failed.len(), st.requested.len())
    }

    pub fn debug_parts(&self) -> String {
        let st = self.st.borrow();
        st.world.parts().iter().filter(|p| p.limb > 0 || p.mesh.is_some()).take(60).map(|p| format!("{}:limb{} [{:.1},{:.1},{:.1}] s[{:.1},{:.1},{:.1}] shape{} mesh{}", p.id, p.limb, p.cf[0], p.cf[1], p.cf[2], p.size[0], p.size[1], p.size[2], p.shape, p.mesh.is_some())).collect::<Vec<_>>().join("\n")
    }

    pub fn set_sky_cube(&self, faces: &web_sys::js_sys::Array) {
        let mut st = self.st.borrow_mut();
        let gl = st.gl.clone();
        if faces.length() != 6 {
            st.sky_cube = None;
            return;
        }
        let t = gl.create_texture();
        gl.bind_texture(G::TEXTURE_CUBE_MAP, t.as_ref());
        for i in 0..6u32 {
            if let Ok(bm) = faces.get(i).dyn_into::<web_sys::ImageBitmap>() {
                let _ = gl.tex_image_2d_with_u32_and_u32_and_image_bitmap(G::TEXTURE_CUBE_MAP_POSITIVE_X + i, 0, G::RGBA as i32, G::RGBA, G::UNSIGNED_BYTE, &bm);
            }
        }
        gl.generate_mipmap(G::TEXTURE_CUBE_MAP);
        gl.tex_parameteri(G::TEXTURE_CUBE_MAP, G::TEXTURE_MIN_FILTER, G::LINEAR_MIPMAP_LINEAR as i32);
        gl.tex_parameteri(G::TEXTURE_CUBE_MAP, G::TEXTURE_MAG_FILTER, G::LINEAR as i32);
        gl.tex_parameteri(G::TEXTURE_CUBE_MAP, G::TEXTURE_WRAP_S, G::CLAMP_TO_EDGE as i32);
        gl.tex_parameteri(G::TEXTURE_CUBE_MAP, G::TEXTURE_WRAP_T, G::CLAMP_TO_EDGE as i32);
        st.sky_cube = t;
    }

    pub fn clear_terrain_chunk(&self, key: &str) {
        self.st.borrow_mut().terrain.remove(key);
    }

    pub fn set_terrain_chunk(&self, key: &str, pos: &[f32], nrm: &[f32], uv: &[f32], info: &[f32], kind: &str) {
        let mut st = self.st.borrow_mut();
        let n = pos.len() / 3;
        let mut data = Vec::with_capacity(n * 8);
        for i in 0..n {
            data.extend_from_slice(&[pos[i * 3], pos[i * 3 + 1], pos[i * 3 + 2], nrm[i * 3], nrm[i * 3 + 1], nrm[i * 3 + 2], uv[i * 2], uv[i * 2 + 1]]);
        }
        let m = make_mesh(&st.gl, &data);
        st.terrain.entry(key.to_string()).or_default().push((m, [info[0], info[1], info[2]], kind.to_string(), info[4] > 0.5, info[3]));
    }

    pub fn debug_atlas(&self) -> Vec<u8> {
        let st = self.st.borrow();
        let gl = st.gl.clone();
        let mut out = vec![0u8; 1024 * 512 * 4];
        let tex = st.textures.iter().find(|(k, _)| k.starts_with("atlas:")).and_then(|(_, v)| v.clone());
        if let Some(t) = tex {
            gl.bind_framebuffer(G::FRAMEBUFFER, st.atlas_fbo.as_ref());
            gl.framebuffer_texture_2d(G::FRAMEBUFFER, G::COLOR_ATTACHMENT0, G::TEXTURE_2D, Some(&t), 0);
            let _ = gl.read_pixels_with_opt_u8_array(0, 0, 1024, 512, G::RGBA, G::UNSIGNED_BYTE, Some(&mut out));
            gl.bind_framebuffer(G::FRAMEBUFFER, None);
        }
        out
    }

    pub fn loaded_meshes(&self) -> usize {
        self.st.borrow().meshes.len()
    }

    pub fn loaded_textures(&self) -> usize {
        self.st.borrow().textures.values().filter(|t| t.is_some()).count()
    }

    pub fn own_pos(&self) -> String {
        match self.st.borrow().ctl.as_ref() {
            Some(c) => format!("[{:.2},{:.2},{:.2}]", c.pos[0], c.pos[1], c.pos[2]),
            None => "null".into(),
        }
    }

    pub fn send(&self, text: &str) {
        let st = self.st.borrow();
        if let Some(ws) = st.ws.as_ref() {
            if ws.ready_state() == 1 {
                let _ = ws.send_with_str(text);
            }
        }
    }

    pub fn inventory(&self) -> String {
        let st = self.st.borrow();
        let items: Vec<serde_json::Value> = st.world.inventory().into_iter().map(|(id, name, eq)| serde_json::json!({"id": id, "name": name, "equipped": eq})).collect();
        serde_json::Value::Array(items).to_string()
    }

    pub fn take_tap(&self) -> String {
        let mut st = self.st.borrow_mut();
        let v: Vec<String> = std::mem::take(&mut st.tap);
        serde_json::Value::Array(v.into_iter().map(serde_json::Value::String).collect()).to_string()
    }

    pub fn camera_pose(&self) -> String {
        let st = self.st.borrow();
        format!("[{},{},{},{},{},{},{}]", st.eye[0], st.eye[1], st.eye[2], st.target[0], st.target[1], st.target[2], st.view_fov)
    }

    pub fn take_remote(&self) -> String {
        let mut st = self.st.borrow_mut();
        let v: Vec<String> = std::mem::take(&mut st.remotes);
        serde_json::Value::Array(v.into_iter().map(serde_json::Value::String).collect()).to_string()
    }

    pub fn ping(&self) {
        if let Some(ws) = &self._ws {
            if ws.ready_state() == 1 {
                let _ = ws.send_with_str("{\"t\":\"ping\",\"x\":1}");
            }
        }
    }

    pub fn status(&self) -> String {
        self.st.borrow().status.clone()
    }

    pub fn ready(&self) -> bool {
        self.st.borrow().world.ready
    }

    pub fn part_count(&self) -> usize {
        self.st.borrow().world.parts().len()
    }

    pub fn era(&self) -> i32 {
        self.st.borrow().world.era
    }

    pub fn boot_counts(&self) -> Vec<u32> {
        let st = self.st.borrow();
        let w = &st.world;
        let mut out = vec![0u32, 0u32];
        let ws = w.insts.values().find(|i| i.class == "Workspace");
        let mut stack: Vec<u32> = ws.map(|i| i.children.clone()).unwrap_or_default();
        while let Some(id) = stack.pop() {
            if let Some(i) = w.insts.get(&id) {
                if swirl_core::classes::is_a(&i.class, "BasePart") {
                    out[0] += 1;
                } else if swirl_core::classes::is_a(&i.class, "JointInstance") {
                    out[1] += 1;
                }
                stack.extend(i.children.iter().copied());
            }
        }
        out
    }

    pub fn frame(&self, now: f64) {
        let mut st = self.st.borrow_mut();
        let w = st.canvas.client_width().max(1) as u32;
        let h = st.canvas.client_height().max(1) as u32;
        if st.canvas.width() != w || st.canvas.height() != h {
            st.canvas.set_width(w);
            st.canvas.set_height(h);
        }
        let gl = st.gl.clone();
        gl.viewport(0, 0, w as i32, h as i32);
        gl.clear_color(0.55, 0.72, 0.93, 1.0);
        gl.clear(G::COLOR_BUFFER_BIT | G::DEPTH_BUFFER_BIT);
        if !st.world.ready {
            return;
        }
        let dt = if st.last_t > 0.0 { ((now - st.last_t) / 1000.0).clamp(0.0, 0.1) as f32 } else { 0.0 };
        st.last_t = now;
        st.world.t = (now / 1000.0) as f32;
        if st.world.own.is_some() && !st.driven {
            if let Some(cf) = st.world.teleport.take() {
                let (ws, jp) = st.world.humanoid_of(st.world.own.as_ref().unwrap().model);
                let mut c = Controller::new([cf[0], cf[1], cf[2]]);
                c.walk_speed = ws;
                c.jump_power = jp;
                st.ctl = Some(c);
                if !st.follow {
                    st.follow = true;
                    st.dist = 14.0;
                    st.fitted = true;
                    st.pitch = 0.35;
                }
            }
            let boxes = st.world.colliders();
            let k = |s: &str| st.keys.contains(s);
            let input = Input {
                forward: (k("w") || k("arrowup")) as i32 as f32 - (k("s") || k("arrowdown")) as i32 as f32,
                right: (k("d") || k("arrowright")) as i32 as f32 - (k("a") || k("arrowleft")) as i32 as f32,
                jump: k(" "),
            };
            let yaw = st.yaw;
            let mut sent: Option<(String, f64)> = None;
            if let Some(c) = st.ctl.as_mut() {
                c.step(dt, &input, yaw, &boxes);
                if c.pos[1] < -400.0 {
                    c.teleport([0.0, 10.0, 0.0]);
                }
            }
            if let Some(c) = st.ctl.as_ref() {
                let cf = mat::cf_yaw(c.pos, c.yaw);
                let (pos, vel, state) = (c.pos, c.vel, c.state);
                st.world.t = (now / 1000.0) as f32;
                let sp = (vel[0] * vel[0] + vel[2] * vel[2]).sqrt();
                st.world.move_own(cf, sp, state);
                st.target = match st.focus {
                    Some(f) => f,
                    None => [pos[0], pos[1] + 1.5, pos[2]],
                };
                st.send_acc += dt as f64;
                if st.send_acc >= 1.0 / 30.0 {
                    st.send_acc = 0.0;
                    let r = |v: f32| (v * 10000.0).round() / 10000.0;
                    let msg = format!(
                        "{{\"t\":\"char\",\"s\":{},\"c\":[{}],\"v\":[{},{},{}],\"st\":{}}}",
                        st.world.tp_seq,
                        cf.iter().map(|x| r(*x).to_string()).collect::<Vec<_>>().join(","),
                        (vel[0] * 100.0).round() / 100.0,
                        (vel[1] * 100.0).round() / 100.0,
                        (vel[2] * 100.0).round() / 100.0,
                        state
                    );
                    sent = Some((msg, 0.0));
                }
            }
            if let (Some((msg, _)), Some(ws)) = (sent, st.ws.as_ref()) {
                if ws.ready_state() == 1 {
                    let _ = ws.send_with_str(&msg);
                }
            }
        }
        let use_cam = st.driven && st.has_cam && st.focus.is_none();
        if !st.fitted && !use_cam {
            let (c, r) = st.world.bounds();
            st.target = c;
            st.dist = r * 1.6;
            st.fitted = true;
        }
        let (eye, dirv, rightv, upv, fov, view) = if use_cam {
            let c = st.cam_cf;
            let eye = [c[0], c[1], c[2]];
            let dirv = [-c[5], -c[8], -c[11]];
            let rightv = [c[3], c[6], c[9]];
            let upv = [c[4], c[7], c[10]];
            let tgt = [eye[0] + dirv[0], eye[1] + dirv[1], eye[2] + dirv[2]];
            st.target = tgt;
            (eye, dirv, rightv, upv, st.cam_fov, mat::look_at(eye, tgt, upv))
        } else {
            let eye = [
                st.target[0] + st.dist * st.pitch.cos() * st.yaw.sin(),
                st.target[1] + st.dist * st.pitch.sin(),
                st.target[2] + st.dist * st.pitch.cos() * st.yaw.cos(),
            ];
            let dirv = norm3(target_minus(st.target, eye));
            let up_ref = if dirv[1].abs() > 0.99 { [0.0, 0.0, 1.0] } else { [0.0, 1.0, 0.0] };
            let rightv = norm3(cross3(dirv, up_ref));
            let upv = cross3(rightv, dirv);
            (eye, dirv, rightv, upv, 1.0, mat::look_at(eye, st.target, [0.0, 1.0, 0.0]))
        };
        st.eye = eye;
        st.view_fov = fov;
        let proj = mat::perspective(fov, w as f32 / h as f32, 0.5, 20000.0);
        let vp = mat::mul(&proj, &view);
        let lt = st.world.lighting();
        let era2018 = st.world.era == 2018;
        let mut wants: Vec<(String, String)> = vec![];
        let sky_ok = st.sky_cube.is_some();
        let parts = st.world.parts();
        let fwd = dirv;
        let mut clothes: HashMap<u32, Option<swirl_net::world::Clothes>> = HashMap::new();
        for p in parts.iter() {
            if (p.limb > 0 || p.r15 > 0) && !clothes.contains_key(&p.model) {
                clothes.insert(p.model, st.world.clothes(p.model));
            }
        }
        let mut r15_groups: Vec<(u32, u8)> = parts.iter().filter(|p| p.r15 > 0).map(|p| (p.model, p.r15)).collect();
        r15_groups.sort();
        r15_groups.dedup();
        for (model, group) in r15_groups {
            if let Some(Some(cl)) = clothes.get(&model) {
                compose_atlas_r15(&mut st, model, group, cl, &mut wants);
            }
        }
        for (model, cl) in clothes.iter() {
            if let Some(cl) = cl {
                compose_atlas(&mut st, *model, cl, &mut wants);
            }
        }
        gl.viewport(0, 0, w as i32, h as i32);
        let items = build_items(&st, &parts, eye, fwd, era2018, &clothes, &mut wants);
        let ssize = st.shadow.size;
        let half = 200.0f32;
        let d = lt.lamp0_dir;
        let texel = (half * 2.0) / ssize as f32;
        let mut rv = [-d[2], 0.0, d[0]];
        let mut rl = (rv[0] * rv[0] + rv[1] * rv[1] + rv[2] * rv[2]).sqrt();
        if rl < 1e-4 {
            rv = [1.0, 0.0, 0.0];
            rl = 1.0;
        }
        rv = [rv[0] / rl, rv[1] / rl, rv[2] / rl];
        let uv_ = [d[1] * rv[2] - d[2] * rv[1], d[2] * rv[0] - d[0] * rv[2], d[0] * rv[1] - d[1] * rv[0]];
        let a = ((eye[0] * rv[0] + eye[1] * rv[1] + eye[2] * rv[2]) / texel).round() * texel;
        let b = ((eye[0] * uv_[0] + eye[1] * uv_[1] + eye[2] * uv_[2]) / texel).round() * texel;
        let c = eye[0] * d[0] + eye[1] * d[1] + eye[2] * d[2];
        let center = [rv[0] * a + uv_[0] * b + d[0] * c, rv[1] * a + uv_[1] * b + d[1] * c, rv[2] * a + uv_[2] * b + d[2] * c];
        let lpos = [center[0] + d[0] * 300.0, center[1] + d[1] * 300.0, center[2] + d[2] * 300.0];
        let mut lup = [0.0f32, 1.0, 0.0];
        if d[0].abs() < 1e-6 && d[2].abs() < 1e-6 {
            lup = [0.0, 0.0, 1.0];
        }
        let lview = mat::look_at(lpos, center, lup);
        let lproj = ortho(-half, half, half, -half, 1.0, 700.0);
        let lvp = mat::mul(&lproj, &lview);
        gl.bind_framebuffer(G::FRAMEBUFFER, st.shadow.fbo.as_ref());
        gl.viewport(0, 0, ssize, ssize);
        gl.depth_mask(true);
        gl.enable(G::DEPTH_TEST);
        gl.disable(G::BLEND);
        gl.clear(G::DEPTH_BUFFER_BIT);
        gl.use_program(Some(&st.dp.prog));
        gl.uniform_matrix4fv_with_f32_array(st.dp.l(&gl, "lvp").as_ref(), false, &lvp);
        gl.enable(G::CULL_FACE);
        gl.cull_face(G::FRONT);
        let dm = st.dp.l(&gl, "m");
        for it in items.iter() {
            if !it.cast {
                continue;
            }
            let mesh = match mesh_of(&st, &it.geo) {
                Some(m) => m,
                None => continue,
            };
            gl.uniform_matrix4fv_with_f32_array(dm.as_ref(), false, &it.m);
            gl.bind_vertex_array(mesh.vao.as_ref());
            gl.draw_arrays(G::TRIANGLES, 0, mesh.count);
        }
        gl.cull_face(G::BACK);
        gl.bind_framebuffer(G::FRAMEBUFFER, None);
        gl.viewport(0, 0, w as i32, h as i32);
        gl.enable(G::BLEND);
        {
            let day = (lt.sun_dir[1] * 4.0 + 0.35).clamp(0.0, 1.0);
            let top = [0.36 * day + 0.02, 0.6 * day + 0.03, 0.95 * day + 0.08];
            let hor = [0.8 * day + 0.05, 0.88 * day + 0.06, 0.98 * day + 0.12];
            let tan_y = (fov / 2.0).tan();
            gl.use_program(Some(&st.sp.prog));
            gl.depth_mask(false);
            gl.disable(G::DEPTH_TEST);
            gl.disable(G::CULL_FACE);
            gl.uniform1i(st.sp.l(&gl, "dome").as_ref(), (!sky_ok) as i32);
            gl.uniform3f(st.sp.l(&gl, "ctop").as_ref(), top[0], top[1], top[2]);
            gl.uniform3f(st.sp.l(&gl, "chor").as_ref(), hor[0], hor[1], hor[2]);
            gl.uniform3f(st.sp.l(&gl, "cbot").as_ref(), 0.55, 0.55, 0.55);
            gl.uniform3f(st.sp.l(&gl, "sunDir").as_ref(), lt.sun_dir[0], lt.sun_dir[1], lt.sun_dir[2]);
            gl.uniform1f(st.sp.l(&gl, "sunVis").as_ref(), day);
            gl.uniform3f(st.sp.l(&gl, "dir").as_ref(), dirv[0], dirv[1], dirv[2]);
            gl.uniform3f(st.sp.l(&gl, "right").as_ref(), rightv[0], rightv[1], rightv[2]);
            gl.uniform3f(st.sp.l(&gl, "up").as_ref(), upv[0], upv[1], upv[2]);
            gl.uniform2f(st.sp.l(&gl, "tanv").as_ref(), tan_y * (w as f32 / h as f32), tan_y);
            gl.uniform1i(st.sp.l(&gl, "cube").as_ref(), 5);
            gl.active_texture(G::TEXTURE5);
            gl.bind_texture(G::TEXTURE_CUBE_MAP, if sky_ok { st.sky_cube.as_ref() } else { st.dummy_cube.as_ref() });
            gl.bind_sampler(5, None);
            gl.bind_vertex_array(st.sky_vao.as_ref());
            gl.draw_arrays(G::TRIANGLES, 0, 3);
            gl.active_texture(G::TEXTURE0);
            gl.enable(G::DEPTH_TEST);
            gl.enable(G::CULL_FACE);
            gl.depth_mask(true);
        }
        let pp = &st.pp;
        gl.use_program(Some(&pp.prog));
        let u = |n: &'static str| pp.l(&gl, n);
        gl.uniform_matrix4fv_with_f32_array(u("vp").as_ref(), false, &vp);
        gl.uniform_matrix4fv_with_f32_array(u("lvp").as_ref(), false, &lvp);
        gl.uniform3f(u("eye").as_ref(), eye[0], eye[1], eye[2]);
        gl.uniform3f(u("fwd").as_ref(), fwd[0], fwd[1], fwd[2]);
        gl.uniform1f(u("nbias").as_ref(), 0.14);
        gl.uniform3f(u("lamp0Dir").as_ref(), lt.lamp0_dir[0], lt.lamp0_dir[1], lt.lamp0_dir[2]);
        gl.uniform3f(u("lamp0Color").as_ref(), lt.lamp0_color[0], lt.lamp0_color[1], lt.lamp0_color[2]);
        gl.uniform3f(u("lamp1Dir").as_ref(), lt.lamp1_dir[0], lt.lamp1_dir[1], lt.lamp1_dir[2]);
        gl.uniform3f(u("lamp1Color").as_ref(), lt.lamp1_color[0], lt.lamp1_color[1], lt.lamp1_color[2]);
        gl.uniform3f(u("ambColor").as_ref(), lt.amb[0], lt.amb[1], lt.amb[2]);
        gl.uniform3f(u("shadowTint").as_ref(), lt.shadow_tint[0], lt.shadow_tint[1], lt.shadow_tint[2]);
        gl.uniform1i(u("era2018").as_ref(), era2018 as i32);
        gl.uniform2f(u("shSize").as_ref(), ssize as f32, ssize as f32);
        match lt.fog {
            Some((fc, fs, fe)) => {
                gl.uniform1i(u("fogOn").as_ref(), 1);
                gl.uniform3f(u("fogColor").as_ref(), fc[0], fc[1], fc[2]);
                gl.uniform2f(u("fogRange").as_ref(), fs, fe);
            }
            None => gl.uniform1i(u("fogOn").as_ref(), 0),
        }
        for (name, unit) in [("map", 0), ("matMap", 1), ("studMap", 2), ("shadowMap", 3), ("envMap", 4)] {
            gl.uniform1i(u(name).as_ref(), unit);
        }
        gl.uniform1i(u("useShadow").as_ref(), 1);
        gl.uniform1i(u("useEnv").as_ref(), 0);
        gl.active_texture(G::TEXTURE3);
        gl.bind_texture(G::TEXTURE_2D, st.shadow.tex.as_ref());
        gl.bind_sampler(3, None);
        gl.active_texture(G::TEXTURE4);
        gl.bind_texture(G::TEXTURE_CUBE_MAP, st.dummy_cube.as_ref());
        gl.active_texture(G::TEXTURE0);
        let stud_id = if era2018 { "c:platform/textures/studs.png" } else { "c:textures/surfacesAlpha.png" };
        let mut order: Vec<usize> = (0..items.len()).collect();
        let (mut opaque, mut trans): (Vec<usize>, Vec<usize>) = order.drain(..).partition(|i| items[*i].alpha >= 1.0 && !matches!(items[*i].geo, Geo::Plane));
        trans.sort_by(|a, b| items[*b].dist.partial_cmp(&items[*a].dist).unwrap_or(std::cmp::Ordering::Equal));
        opaque.sort_by(|a, b| items[*a].dist.partial_cmp(&items[*b].dist).unwrap_or(std::cmp::Ordering::Equal));
        let mut studs_tex = false;
        match st.textures.get(stud_id) {
            Some(Some(t)) => {
                gl.active_texture(G::TEXTURE2);
                gl.bind_texture(G::TEXTURE_2D, Some(t));
                gl.bind_sampler(2, st.samp_repeat.as_ref());
                gl.active_texture(G::TEXTURE0);
                studs_tex = true;
            }
            Some(None) => {}
            None => wants.push(("tex".into(), stud_id.to_string())),
        }
        for idx in opaque.iter().chain(trans.iter()) {
            let it = &items[*idx];
            let mesh = match mesh_of(&st, &it.geo) {
                Some(m) => m,
                None => continue,
            };
            let mut map_on = 0;
            if let Some(id) = &it.map {
                match st.textures.get(id) {
                    Some(Some(t)) => {
                        gl.active_texture(G::TEXTURE0);
                        gl.bind_texture(G::TEXTURE_2D, Some(t));
                        gl.bind_sampler(0, if it.map_repeat { st.samp_repeat.as_ref() } else { st.samp_clamp.as_ref() });
                        map_on = 1;
                    }
                    Some(None) => {}
                    None => wants.push(("tex".into(), id.clone())),
                }
            }
            let mut mat_mode = -1;
            if let Some(md) = &it.mat {
                if let Some(tid) = &md.tex {
                    match st.textures.get(tid) {
                        Some(Some(t)) => {
                            gl.active_texture(G::TEXTURE1);
                            gl.bind_texture(G::TEXTURE_2D, Some(t));
                            gl.bind_sampler(1, st.samp_repeat.as_ref());
                            gl.active_texture(G::TEXTURE0);
                            mat_mode = if md.blend { 1 } else if md.mul { 2 } else { 0 };
                            gl.uniform4f(u("matU").as_ref(), md.u[0], md.u[1], md.u[2], md.u[3]);
                            gl.uniform4f(u("matV").as_ref(), md.v[0], md.v[1], md.v[2], md.v[3]);
                        }
                        Some(None) => {}
                        None => wants.push(("tex".into(), tid.clone())),
                    }
                }
            }
            gl.uniform1i(u("useMap").as_ref(), map_on);
            gl.uniform1i(u("matMode").as_ref(), mat_mode);
            gl.uniform_matrix4fv_with_f32_array(u("m").as_ref(), false, &it.m);
            gl.uniform3f(u("psz").as_ref(), it.psz[0], it.psz[1], it.psz[2]);
            gl.uniform1i(u("umode").as_ref(), it.mode);
            gl.uniform2f(u("uvs").as_ref(), it.uvs[0], it.uvs[1]);
            gl.uniform3f(u("diffuse").as_ref(), it.color[0], it.color[1], it.color[2]);
            gl.uniform1f(u("alpha").as_ref(), it.alpha);
            gl.uniform1f(u("ks").as_ref(), it.ks);
            gl.uniform1f(u("specExp").as_ref(), it.exp);
            gl.uniform1f(u("cutoff").as_ref(), it.cutoff);
            gl.uniform1i(u("neon").as_ref(), it.neon as i32);
            gl.uniform1i(u("studOnly").as_ref(), it.stud_only as i32);
            match (&it.studs, studs_tex) {
                (Some(rows), true) => {
                    gl.uniform1i(u("useStuds").as_ref(), 1);
                    gl.uniform1fv_with_f32_array(u("studRow").as_ref(), rows);
                }
                _ => gl.uniform1i(u("useStuds").as_ref(), 0),
            }
            if it.stud_only && !studs_tex {
                continue;
            }
            let transparent = it.alpha < 1.0 || matches!(it.geo, Geo::Plane);
            gl.depth_mask(!(transparent && it.alpha <= 0.5) && !matches!(it.geo, Geo::Plane) && !(transparent && matches!(it.geo, Geo::Terrain(..))));
            if it.cull {
                gl.enable(G::CULL_FACE);
            } else {
                gl.disable(G::CULL_FACE);
            }
            if it.offset {
                gl.enable(G::POLYGON_OFFSET_FILL);
                gl.polygon_offset(-2.0, -2.0);
            } else {
                gl.disable(G::POLYGON_OFFSET_FILL);
            }
            gl.bind_vertex_array(mesh.vao.as_ref());
            gl.draw_arrays(G::TRIANGLES, 0, mesh.count);
        }
        gl.disable(G::POLYGON_OFFSET_FILL);
        gl.depth_mask(true);
        gl.enable(G::CULL_FACE);
        drop(u);
        for (k, id) in wants {
            let key = format!("{}:{}", k, id);
            if st.requested.insert(key) {
                st.queue.push((k, id));
            }
        }
    }
}

fn target_minus(t: [f32; 3], e: [f32; 3]) -> [f32; 3] {
    [t[0] - e[0], t[1] - e[1], t[2] - e[2]]
}

fn norm3(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-9);
    [v[0] / l, v[1] / l, v[2] / l]
}

fn cross3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

unsafe fn js_sys_f32(data: &[f32]) -> web_sys::js_sys::Float32Array {
    web_sys::js_sys::Float32Array::view(data)
}

fn ortho(l: f32, r: f32, t: f32, b: f32, n: f32, f: f32) -> M4 {
    [2.0 / (r - l), 0.0, 0.0, 0.0, 0.0, 2.0 / (t - b), 0.0, 0.0, 0.0, 0.0, -2.0 / (f - n), 0.0, -(r + l) / (r - l), -(t + b) / (t - b), -(f + n) / (f - n), 1.0]
}

fn mesh_of<'a>(st: &'a State, g: &Geo) -> Option<&'a Mesh> {
    match g {
        Geo::Shape(i) => st.shapes.get(*i),
        Geo::Plane => Some(&st.plane),
        Geo::Terrain(k, i) => st.terrain.get(k).and_then(|v| v.get(*i)).map(|t| &t.0),
        Geo::Mesh(k) => st.meshes.get(k),
    }
}

fn kind_of(material: u16) -> &'static str {
    match material {
        256 => "plastic",
        272 => "smoothplastic",
        288 => "neon",
        512 => "wood",
        528 => "woodplanks",
        784 | 800 => "slate",
        816 => "concrete",
        832 => "granite",
        848 => "brick",
        864 => "pebble",
        880 => "cobblestone",
        1040 => "corrodedmetal",
        1056 => "diamondplate",
        1072 => "foil",
        1088 => "metal",
        1280 => "grass",
        1296 => "sand",
        1312 => "fabric",
        1536 => "ice",
        1568 => "glass",
        _ => "plastic",
    }
}

fn matdef(era2018: bool, material: u16, truss: bool) -> MatDef {
    matdef_kind(era2018, if truss { "truss" } else { kind_of(material) })
}

fn matdef_kind(era2018: bool, kind: &str) -> MatDef {
    let base = |ks: f32, exp: f32| MatDef { ks, exp, tex: None, u: [0.0; 4], v: [0.0; 4], blend: false, mul: false, neon: false };
    if era2018 {
        let t18 = |ks: f32, exp: f32, name: &str, scale: f32| MatDef { ks, exp, tex: Some(format!("c:platform/textures/{}/diffuse.png", name)), u: [1.0 / scale, 0.0, 0.0, 0.0], v: [0.0, 1.0 / scale, 0.0, 0.0], blend: false, mul: true, neon: false };
        return match kind {
            "smoothplastic" => base(0.25, 60.0),
            "neon" => MatDef { ks: 0.0, exp: 1.0, neon: true, ..base(0.0, 1.0) },
            "fabric" => t18(0.05, 10.0, "fabric", 4.0),
            "wood" => t18(0.15, 8.0, "wood", 8.0),
            "woodplanks" => t18(0.15, 8.0, "wood", 6.0),
            "slate" => t18(0.1, 20.0, "slate", 8.0),
            "granite" => t18(0.1, 20.0, "granite", 8.0),
            "brick" => t18(0.05, 10.0, "brick", 8.0),
            "pebble" => t18(0.1, 20.0, "pebble", 6.0),
            "cobblestone" => t18(0.1, 20.0, "cobblestone", 8.0),
            "concrete" => t18(0.1, 14.0, "concrete", 8.0),
            "sand" => t18(0.05, 10.0, "sand", 6.0),
            "corrodedmetal" => t18(0.2, 40.0, "rust", 8.0),
            "diamondplate" => t18(0.5, 50.0, "diamondplate", 4.0),
            "foil" => t18(0.6, 60.0, "aluminum", 8.0),
            "metal" => t18(0.5, 50.0, "metal", 8.0),
            "grass" => t18(0.05, 10.0, "grass", 8.0),
            "ice" => t18(0.6, 80.0, "ice", 8.0),
            "glass" => base(0.8, 120.0),
            _ => base(0.35, 40.0),
        };
    }
    let t12 = |ks: f32, exp: f32, tex: &str, u: [f32; 4], v: [f32; 4], blend: bool| MatDef { ks, exp, tex: Some(format!("c:textures/{}", tex)), u, v, blend, mul: false, neon: false };
    let wood = [0.01699672, 0.1353425, -0.03733006, 0.0];
    let woodv = [-0.1155417, -0.07062286, -0.04077413, 0.0];
    let s28 = ([0.28, 0.0, 0.0, 0.0], [0.0, 0.28, 0.0, 0.0]);
    let s18 = ([0.18, 0.0, 0.0, 0.0], [0.0, 0.18, 0.0, 0.0]);
    let s06 = ([0.06, 0.0, 0.0, 0.0], [0.0, 0.06, 0.0, 0.0]);
    match kind {
        "neon" => MatDef { ks: 0.0, exp: 1.0, neon: true, ..base(0.0, 1.0) },
        "fabric" => base(0.1, 20.0),
        "wood" | "woodplanks" => t12(0.2696, 4.0, "WoodFallback.png", wood, woodv, false),
        "slate" | "granite" | "brick" | "pebble" | "cobblestone" => t12(0.1, 40.0, "SlateTileMono.png", s28.0, s28.1, false),
        "concrete" | "sand" => t12(0.19, 14.0, "ConcreteFallback.png", s18.0, s18.1, false),
        "corrodedmetal" => t12(0.12, 70.0, "rust_combined.png", [0.05, 0.0376, 0.0, 0.0], [-0.0376, 0.05, 0.0, 0.0], true),
        "diamondplate" => t12(0.0, 25.0, "DiamondPlateFallback.png", s28.0, s28.1, false),
        "foil" | "metal" => t12(0.0, 25.0, "AluminumFallback.png", s06.0, s06.1, false),
        "grass" => t12(0.1, 50.0, "Grass_Texture_gray.jpg", [0.1, 0.0, 0.0, 0.0], [0.0, 0.1, 0.0, 0.0], false),
        "ice" => t12(0.0, 25.0, "IceFallback.png", s06.0, s06.1, false),
        _ => base(0.75, 81.0),
    }
}

fn stud_rows(surf: &[u8; 6], era2018: bool) -> Option<[f32; 6]> {
    let mut rows = [-1.0f32; 6];
    let mut any = false;
    for (i, s) in surf.iter().enumerate() {
        let row = match s {
            1 => 3,
            2 => 2,
            3 => 0,
            4 => 1,
            5 => 4,
            _ => continue,
        };
        let row = if era2018 { [0, 2, 1, 1, 3][row] } else { row };
        rows[i] = row as f32;
        any = true;
    }
    if any {
        Some(rows)
    } else {
        None
    }
}

fn euler_xyz(x: f32, y: f32, z: f32) -> [f32; 9] {
    let (a, b, c, d, e, f) = (x.cos(), x.sin(), y.cos(), y.sin(), z.cos(), z.sin());
    let (ae, af, be, bf) = (a * e, a * f, b * e, b * f);
    [c * e, af + be * d, bf - ae * d, -c * f, ae - bf * d, be + af * d, d, -b * c, a * c]
}

fn local_matrix(rot: &[f32; 9], pos: [f32; 3], scale: [f32; 3]) -> M4 {
    [
        rot[0] * scale[0], rot[1] * scale[0], rot[2] * scale[0], 0.0,
        rot[3] * scale[1], rot[4] * scale[1], rot[5] * scale[1], 0.0,
        rot[6] * scale[2], rot[7] * scale[2], rot[8] * scale[2], 0.0,
        pos[0], pos[1], pos[2], 1.0,
    ]
}

fn build_items(st: &State, parts: &[swirl_net::world::Part], eye: [f32; 3], fwd: [f32; 3], era2018: bool, clothes: &HashMap<u32, Option<swirl_net::world::Clothes>>, wants: &mut Vec<(String, String)>) -> Vec<Item> {
    let mut out: Vec<Item> = vec![];
    for (key, groups) in st.terrain.iter() {
        for (i, (_, color, kind, water, opacity)) in groups.iter().enumerate() {
            let def = matdef_kind(false, kind);
            out.push(Item {
                geo: Geo::Terrain(key.clone(), i),
                m: mat::identity(),
                psz: [1.0; 3],
                mode: 3,
                uvs: [1.0, 1.0],
                color: *color,
                alpha: *opacity,
                ks: if *water { 0.8 } else { def.ks },
                exp: if *water { 80.0 } else { def.exp },
                cutoff: 0.0,
                neon: false,
                map: None,
                map_repeat: false,
                mat: if *water || era2018 { None } else if def.tex.is_some() { Some(def) } else { None },
                studs: None,
                stud_only: false,
                cast: !*water,
                receive: true,
                cull: true,
                offset: false,
                dist: if *water { 0.0 } else { 1.0e6 },
            });
        }
    }
    for p in parts {
        let base = mat::from_cframe(&p.cf, [1.0, 1.0, 1.0]);
        let dist = (p.cf[0] - eye[0]) * fwd[0] + (p.cf[1] - eye[1]) * fwd[1] + (p.cf[2] - eye[2]) * fwd[2];
        let mut body_m: Option<M4> = None;
        if p.alpha > 0.0 {
            let item = |geo: Geo, m: M4| Item {
                geo,
                m,
                psz: p.size,
                mode: 2,
                uvs: [1.0, 1.0],
                color: p.color,
                alpha: p.alpha,
                ks: 0.5,
                exp: 40.0,
                cutoff: 0.0,
                neon: false,
                map: None,
                map_repeat: false,
                mat: None,
                studs: None,
                stud_only: false,
                cast: p.cast,
                receive: true,
                cull: true,
                offset: false,
                dist,
            };
            if let Some(mr) = &p.mesh {
                let key = mr.mesh.clone();
                if let Some(mesh) = st.meshes.get(&key) {
                    let ext = [(mesh.hi[0] - mesh.lo[0]).max(0.001), (mesh.hi[1] - mesh.lo[1]).max(0.001), (mesh.hi[2] - mesh.lo[2]).max(0.001)];
                    let c = [(mesh.hi[0] + mesh.lo[0]) / 2.0, (mesh.hi[1] + mesh.lo[1]) / 2.0, (mesh.hi[2] + mesh.lo[2]) / 2.0];
                    let (s, o) = if let Some(u) = mr.unit {
                        let s = [u[0] / ext[0], u[1] / ext[1], u[2] / ext[2]];
                        (s, [-c[0] * s[0], -c[1] * s[1], -c[2] * s[2]])
                    } else if mr.fit {
                        let s = [p.size[0] / ext[0], p.size[1] / ext[1], p.size[2] / ext[2]];
                        (s, [-c[0] * s[0], -c[1] * s[1], -c[2] * s[2]])
                    } else {
                        (mr.scale, mr.off)
                    };
                    let local = [s[0], 0.0, 0.0, 0.0, 0.0, s[1], 0.0, 0.0, 0.0, 0.0, s[2], 0.0, o[0], o[1], o[2], 1.0];
                    let m = mat::mul(&base, &local);
                    body_m = Some(m);
                    let mut it = item(Geo::Mesh(key), m);
                    if let Some(tid) = &mr.tex {
                        it.color = mr.vc;
                        match st.textures.get(tid) {
                            Some(Some(_)) => {
                                it.map = Some(tid.clone());
                                it.cutoff = 0.1;
                            }
                            Some(None) => it.color = p.color,
                            None => wants.push(("tex".into(), tid.clone())),
                        }
                    }
                    let mut over: Option<Item> = None;
                    if p.r15 > 0 {
                        let aid = format!("atlas:{}:{}", p.model, p.r15);
                        if let Some(Some(_)) = st.textures.get(&aid) {
                            if let (Some(tid), Some(Some(_))) = (&mr.tex, mr.tex.as_ref().and_then(|t| st.textures.get(t))) {
                                let mut o = item(Geo::Mesh(mr.mesh.clone()), m);
                                o.color = [1.0, 1.0, 1.0];
                                o.map = Some(tid.clone());
                                o.cutoff = 0.1;
                                o.offset = true;
                                over = Some(o);
                            }
                            it.color = [1.0, 1.0, 1.0];
                            it.map = Some(aid);
                            it.cutoff = 0.0;
                        }
                    }
                    out.push(it);
                    if let Some(o) = over {
                        out.push(o);
                    }
                } else if !st.failed.contains(&key) {
                    wants.push(("mesh".into(), key));
                }
            } else if p.limb > 0 {
                let (path, ls) = match p.limb {
                    1 => ("torso", [2.0f32, 2.0, 1.0]),
                    2 => ("leftarm", [1.0, 2.0, 1.0]),
                    3 => ("rightarm", [1.0, 2.0, 1.0]),
                    4 => ("leftleg", [1.0, 2.0, 1.0]),
                    _ => ("rightleg", [1.0, 2.0, 1.0]),
                };
                let id = format!("c:fonts/{}.mesh", path);
                let key = format!("{}#flat", id);
                if st.meshes.contains_key(&key) {
                    let local = [p.size[0] / ls[0], 0.0, 0.0, 0.0, 0.0, p.size[1] / ls[1], 0.0, 0.0, 0.0, 0.0, p.size[2] / ls[2], 0.0, 0.0, 0.0, 0.0, 1.0];
                    let m = mat::mul(&base, &local);
                    let mut it = item(Geo::Mesh(key), m);
                    it.ks = 0.3;
                    it.exp = 30.0;
                    let aid = format!("atlas:{}", p.model);
                    if let Some(Some(_)) = st.textures.get(&aid) {
                        it.color = [1.0, 1.0, 1.0];
                        it.map = Some(aid);
                    }
                    out.push(it);
                } else if !st.failed.contains(&id) {
                    wants.push(("mesh".into(), id));
                }
                let has_shirt = clothes.get(&p.model).and_then(|c| c.as_ref()).map(|c| c.shirt.is_some()).unwrap_or(false);
                if !era2018 && !has_shirt && p.limb <= 3 {
                    let mut faces: Vec<(f32, f32, bool)> = vec![(p.size[1] / 2.0, -std::f32::consts::FRAC_PI_2, false)];
                    if p.limb != 1 {
                        faces.push((-p.size[1] / 2.0, std::f32::consts::FRAC_PI_2, true));
                    }
                    for (y, rx, solid) in faces {
                        let k = if solid { 1.0 } else { 0.94 };
                        let (w, h) = (p.size[0] * k, p.size[2] * k);
                        let pos = [0.0, y + y.signum() * 0.004, 0.0];
                        let m = mat::mul(&base, &local_matrix(&euler_xyz(rx, 0.0, 0.0), pos, [w, h, 1.0]));
                        let mut it = item(Geo::Plane, m);
                        it.uvs = [w, h];
                        it.ks = 0.3;
                        it.exp = 30.0;
                        it.studs = Some([if solid { 1.0 } else { 0.0 }; 6]);
                        it.stud_only = !solid;
                        it.offset = true;
                        it.cast = false;
                        if !solid {
                            it.alpha = p.alpha.min(1.0);
                        }
                        out.push(it);
                    }
                }
            } else {
                let m = mat::from_cframe(&p.cf, p.size);
                body_m = Some(m);
                let gi = match p.shape {
                    0 => 0,
                    2 => 2,
                    3 => 3,
                    4 => 4,
                    5 => 5,
                    _ => 1,
                };
                let mut it = item(Geo::Shape(gi), m);
                it.mode = if gi == 1 { 0 } else { 1 };
                if p.dmesh {
                } else {
                    let def = matdef(era2018, p.material, p.truss);
                    it.ks = def.ks;
                    it.exp = def.exp;
                    it.neon = def.neon;
                    if gi == 1 && !p.truss && !def.neon {
                        it.studs = stud_rows(&p.surf, era2018);
                    }
                    if def.tex.is_some() {
                        it.mat = Some(def);
                    }
                }
                out.push(it);
            }
        }
        for d in &p.decals {
            let (geo, m, uvs) = if p.head_face {
                if d.face != 5 {
                    continue;
                }
                match body_m {
                    Some(bm) => (Geo::Mesh("c:fonts/head.mesh#face".into()), mat::mul(&bm, &[1.002, 0.0, 0.0, 0.0, 0.0, 1.002, 0.0, 0.0, 0.0, 0.0, 1.002, 0.0, 0.0, 0.0, 0.0, 1.0]), [1.0, 1.0]),
                    None => continue,
                }
            } else if p.mesh.is_some() {
                continue;
            } else {
                let s = p.size;
                let h = [s[0] / 2.0, s[1] / 2.0, s[2] / 2.0];
                let e = 0.002;
                let dims = match d.face {
                    0 | 3 => [s[2], s[1]],
                    1 | 4 => [s[0], s[2]],
                    _ => [s[0], s[1]],
                };
                let pi = std::f32::consts::PI;
                let (pos, rot) = match d.face {
                    0 => ([h[0] + e, 0.0, 0.0], euler_xyz(0.0, pi / 2.0, 0.0)),
                    1 => ([0.0, h[1] + e, 0.0], euler_xyz(-pi / 2.0, 0.0, pi)),
                    2 => ([0.0, 0.0, h[2] + e], euler_xyz(0.0, 0.0, 0.0)),
                    3 => ([-h[0] - e, 0.0, 0.0], euler_xyz(0.0, -pi / 2.0, 0.0)),
                    4 => ([0.0, -h[1] - e, 0.0], euler_xyz(pi / 2.0, 0.0, 0.0)),
                    _ => ([0.0, 0.0, -h[2] - e], euler_xyz(0.0, pi, 0.0)),
                };
                let uvs = match d.tile {
                    Some(t) => [dims[0] / t[0], dims[1] / t[1]],
                    None => [1.0, 1.0],
                };
                (Geo::Plane, mat::mul(&base, &local_matrix(&rot, pos, [dims[0], dims[1], 1.0])), uvs)
            };
            let tex_ok = match st.textures.get(&d.tex) {
                Some(Some(_)) => true,
                Some(None) => false,
                None => {
                    wants.push(("tex".into(), d.tex.clone()));
                    false
                }
            };
            if !tex_ok {
                continue;
            }
            out.push(Item {
                geo,
                m,
                psz: p.size,
                mode: 2,
                uvs,
                color: d.color,
                alpha: 1.0 - d.transp,
                ks: (d.spec * 0.5).min(1.0),
                exp: d.shiny,
                cutoff: 0.0,
                neon: false,
                map: Some(d.tex.clone()),
                map_repeat: d.tile.is_some(),
                mat: None,
                studs: None,
                stud_only: false,
                cast: false,
                receive: true,
                cull: true,
                offset: true,
                dist,
            });
        }
    }
    out
}

fn lin(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn compose_atlas_r15(st: &mut State, model: u32, group: u8, cl: &swirl_net::world::Clothes, wants: &mut Vec<(String, String)>) {
    let (base, w, h) = match group {
        1 => ("c:avatar/compositing/R15CompositTorsoBase.mesh", 388.0f32, 272.0f32),
        2 | 4 => ("c:avatar/compositing/R15CompositLeftArmBase.mesh", 264.0, 284.0),
        _ => ("c:avatar/compositing/R15CompositRightArmBase.mesh", 264.0, 284.0),
    };
    let layers: Vec<&Option<String>> = match group {
        1 => vec![&cl.pants, &cl.shirt],
        2 | 3 => vec![&cl.shirt],
        _ => vec![&cl.pants],
    };
    let tint = cl.r15_colors[(group - 1) as usize];
    let mut key = format!("{:?}{:?}", layers, tint);
    if !st.meshes.contains_key(base) {
        if !st.failed.contains(base) {
            wants.push(("mesh".into(), base.to_string()));
        }
        return;
    }
    let mut ready: Vec<Option<String>> = vec![];
    for l in layers.iter() {
        match l {
            Some(t) => {
                let ok = matches!(st.textures.get(t), Some(Some(_)));
                if st.textures.get(t).is_none() {
                    wants.push(("tex".into(), t.clone()));
                }
                key.push(if ok { '1' } else { '0' });
                ready.push(if ok { Some(t.clone()) } else { None });
            }
            None => ready.push(None),
        }
    }
    if st.atlases_r15.get(&(model, group)) == Some(&key) {
        return;
    }
    let gl = st.gl.clone();
    let (tw, th) = ((w * 2.0) as i32, (h * 2.0) as i32);
    let tid = format!("atlas:{}:{}", model, group);
    let tex = match st.textures.get(&tid) {
        Some(Some(t)) => t.clone(),
        _ => {
            let t = gl.create_texture();
            gl.bind_texture(G::TEXTURE_2D, t.as_ref());
            gl.tex_storage_2d(G::TEXTURE_2D, 10, G::SRGB8_ALPHA8, tw, th);
            st.textures.insert(tid.clone(), t.clone());
            t.unwrap()
        }
    };
    gl.active_texture(G::TEXTURE0);
    gl.bind_texture(G::TEXTURE_2D, None);
    gl.bind_framebuffer(G::FRAMEBUFFER, st.atlas_fbo.as_ref());
    gl.framebuffer_texture_2d(G::FRAMEBUFFER, G::COLOR_ATTACHMENT0, G::TEXTURE_2D, Some(&tex), 0);
    gl.viewport(0, 0, tw, th);
    gl.clear_color(lin(tint[0]), lin(tint[1]), lin(tint[2]), 1.0);
    gl.clear(G::COLOR_BUFFER_BIT);
    gl.disable(G::DEPTH_TEST);
    gl.disable(G::CULL_FACE);
    gl.enable(G::BLEND);
    gl.use_program(Some(&st.cp.prog));
    let vp = ortho(0.0, w, h, 0.0, -1000.0, 1000.0);
    gl.uniform_matrix4fv_with_f32_array(st.cp.l(&gl, "vp").as_ref(), false, &vp);
    gl.uniform1i(st.cp.l(&gl, "tex").as_ref(), 0);
    gl.uniform1i(st.cp.l(&gl, "useTex").as_ref(), 1);
    if let Some(m) = st.meshes.get(base) {
        for t in ready.iter().flatten() {
            if let Some(Some(img)) = st.textures.get(t) {
                gl.active_texture(G::TEXTURE0);
                gl.bind_texture(G::TEXTURE_2D, Some(img));
                gl.bind_sampler(0, st.samp_clamp.as_ref());
                gl.bind_vertex_array(m.vao.as_ref());
                gl.draw_arrays(G::TRIANGLES, 0, m.count);
            }
        }
    }
    gl.bind_framebuffer(G::FRAMEBUFFER, None);
    gl.bind_texture(G::TEXTURE_2D, Some(&tex));
    gl.generate_mipmap(G::TEXTURE_2D);
    gl.bind_texture(G::TEXTURE_2D, None);
    gl.enable(G::DEPTH_TEST);
    gl.enable(G::CULL_FACE);
    st.atlases_r15.insert((model, group), key);
}

fn compose_atlas(st: &mut State, model: u32, cl: &swirl_net::world::Clothes, wants: &mut Vec<(String, String)>) {
    let bases = ["c:fonts/CompositTorsoBase.mesh", "c:fonts/CompositLeftArmBase.mesh", "c:fonts/CompositRightArmBase.mesh", "c:fonts/CompositLeftLegBase.mesh", "c:fonts/CompositRightLegBase.mesh"];
    let layers: [(&Option<String>, &str); 3] = [(&cl.pants, "c:fonts/CompositPantsTemplate.mesh"), (&cl.shirt, "c:fonts/CompositShirtTemplate.mesh"), (&cl.tshirt, "c:fonts/CompositTShirt.mesh")];
    let mut key = format!("{:?}{:?}{:?}{:?}", cl.shirt, cl.pants, cl.tshirt, cl.colors);
    let mut bases_ready = true;
    for b in bases.iter() {
        if st.meshes.contains_key(*b) {
            key.push('1');
        } else if st.failed.contains(*b) {
            key.push('f');
        } else {
            key.push('0');
            bases_ready = false;
            wants.push(("mesh".into(), b.to_string()));
        }
    }
    let mut layer_ready = [false; 3];
    for (i, (tid, mid)) in layers.iter().enumerate() {
        if let Some(t) = tid {
            let m_ok = st.meshes.contains_key(*mid);
            if !m_ok && !st.failed.contains(*mid) {
                wants.push(("mesh".into(), mid.to_string()));
            }
            let t_ok = matches!(st.textures.get(t), Some(Some(_)));
            if st.textures.get(t).is_none() {
                wants.push(("tex".into(), t.clone()));
            }
            layer_ready[i] = m_ok && t_ok;
            key.push(if layer_ready[i] { '1' } else { '0' });
        }
    }
    if !bases_ready || st.atlases.get(&model) == Some(&key) {
        return;
    }
    let gl = st.gl.clone();
    let tid = format!("atlas:{}", model);
    let tex = match st.textures.get(&tid) {
        Some(Some(t)) => t.clone(),
        _ => {
            let t = gl.create_texture();
            gl.bind_texture(G::TEXTURE_2D, t.as_ref());
            gl.tex_storage_2d(G::TEXTURE_2D, 11, G::SRGB8_ALPHA8, 1024, 512);
            st.textures.insert(tid.clone(), t.clone());
            t.unwrap()
        }
    };
    gl.active_texture(G::TEXTURE0);
    gl.bind_texture(G::TEXTURE_2D, None);
    gl.bind_framebuffer(G::FRAMEBUFFER, st.atlas_fbo.as_ref());
    gl.framebuffer_texture_2d(G::FRAMEBUFFER, G::COLOR_ATTACHMENT0, G::TEXTURE_2D, Some(&tex), 0);
    gl.viewport(0, 0, 1024, 512);
    gl.clear_color(0.0, 0.0, 0.0, 0.0);
    gl.clear(G::COLOR_BUFFER_BIT);
    gl.disable(G::DEPTH_TEST);
    gl.disable(G::CULL_FACE);
    gl.enable(G::BLEND);
    gl.use_program(Some(&st.cp.prog));
    let vp = ortho(0.0, 1024.0, 512.0, 0.0, -1000.0, 1000.0);
    gl.uniform_matrix4fv_with_f32_array(st.cp.l(&gl, "vp").as_ref(), false, &vp);
    gl.uniform1i(st.cp.l(&gl, "tex").as_ref(), 0);
    gl.uniform1i(st.cp.l(&gl, "useTex").as_ref(), 0);
    for (i, b) in bases.iter().enumerate() {
        if let Some(m) = st.meshes.get(*b) {
            let c = cl.colors[i];
            gl.uniform4f(st.cp.l(&gl, "col").as_ref(), lin(c[0]), lin(c[1]), lin(c[2]), 1.0);
            gl.bind_vertex_array(m.vao.as_ref());
            gl.draw_arrays(G::TRIANGLES, 0, m.count);
        }
    }
    gl.uniform1i(st.cp.l(&gl, "useTex").as_ref(), 1);
    for (i, (tid, mid)) in layers.iter().enumerate() {
        if !layer_ready[i] {
            continue;
        }
        if let (Some(t), Some(m)) = (tid.as_ref().and_then(|t| st.textures.get(t)).and_then(|t| t.as_ref()), st.meshes.get(*mid)) {
            gl.active_texture(G::TEXTURE0);
            gl.bind_texture(G::TEXTURE_2D, Some(t));
            gl.bind_sampler(0, st.samp_clamp.as_ref());
            gl.bind_vertex_array(m.vao.as_ref());
            gl.draw_arrays(G::TRIANGLES, 0, m.count);
        }
    }
    gl.bind_framebuffer(G::FRAMEBUFFER, None);
    gl.bind_texture(G::TEXTURE_2D, Some(&tex));
    gl.generate_mipmap(G::TEXTURE_2D);
    gl.bind_texture(G::TEXTURE_2D, None);
    gl.enable(G::DEPTH_TEST);
    gl.enable(G::CULL_FACE);
    st.atlases.insert(model, key);
}
