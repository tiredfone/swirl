use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use swirl_net::controller::{Controller, Input};
use swirl_net::mat;
use swirl_net::world::World;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{HtmlCanvasElement, WebGl2RenderingContext as G, WebGlProgram, WebGlUniformLocation, WebSocket};

const VS: &str = "#version 300 es
in vec3 p; in vec3 n; in vec2 uv;
uniform mat4 vp; uniform mat4 m; uniform vec4 uvr;
out vec3 vn; out vec2 vuv;
void main(){ vn = mat3(m) * n; vuv = uv * uvr.zw + uvr.xy; gl_Position = vp * m * vec4(p,1.0); }";
const FS: &str = "#version 300 es
precision mediump float;
in vec3 vn; in vec2 vuv; uniform vec4 col; uniform sampler2D tex; uniform float useTex; uniform vec4 lt; uniform vec3 tint; out vec4 o;
void main(){ vec3 N = normalize(vn); float d = max(dot(N, normalize(vec3(0.45,0.8,-0.4))), 0.0); float f = max(dot(N, normalize(vec3(-0.5,0.2,0.6))), 0.0);
float l = min(1.17, lt.x + lt.y*d + lt.z*f); vec4 c = col; if (useTex > 0.5) { vec4 t = texture(tex, vuv); if (useTex > 1.5) { c = vec4(t.rgb, t.a * col.a); } else { c = vec4(mix(col.rgb, t.rgb, t.a), col.a); } } o = vec4(min(c.rgb * l * tint, vec3(1.0)), c.a); }";

const SKY_VS: &str = "#version 300 es
out vec2 q;
void main(){ vec2 p = vec2(float((gl_VertexID << 1) & 2), float(gl_VertexID & 2)); q = p * 2.0 - 1.0; gl_Position = vec4(q, 0.9999, 1.0); }";
const SKY_FS: &str = "#version 300 es
precision mediump float;
in vec2 q; out vec4 o;
uniform vec3 dir; uniform vec3 right; uniform vec3 up; uniform vec2 tanv;
uniform sampler2D f0; uniform sampler2D f1; uniform sampler2D f2; uniform sampler2D f3; uniform sampler2D f4; uniform sampler2D f5;
vec4 pick(int face, vec2 st){ if (face == 0) return texture(f0, st); if (face == 1) return texture(f1, st); if (face == 2) return texture(f2, st); if (face == 3) return texture(f3, st); if (face == 4) return texture(f4, st); return texture(f5, st); }
void main(){
  vec3 r = dir + q.x * tanv.x * right + q.y * tanv.y * up;
  float rx = -r.x; float ry = r.y; float rz = r.z;
  float ax = abs(rx); float ay = abs(ry); float az = abs(rz);
  int face; float sc; float tc; float ma;
  if (ax >= ay && ax >= az) { ma = ax; if (rx > 0.0) { face = 0; sc = -rz; tc = -ry; } else { face = 1; sc = rz; tc = -ry; } }
  else if (ay >= az) { ma = ay; if (ry > 0.0) { face = 2; sc = rx; tc = rz; } else { face = 3; sc = rx; tc = -rz; } }
  else { ma = az; if (rz > 0.0) { face = 4; sc = rx; tc = -ry; } else { face = 5; sc = -rx; tc = -ry; } }
  float s = (sc / ma + 1.0) / 2.0; float t = (tc / ma + 1.0) / 2.0;
  if (face == 2) { float ns = 1.0 - t; t = s; s = ns; } else if (face == 3) { float ns = t; t = 1.0 - s; s = ns; }
  o = vec4(pick(face, vec2(s, t)).rgb, 1.0);
}";

struct State {
    sky_prog: WebGlProgram,
    sky_vao: Option<web_sys::WebGlVertexArrayObject>,
    u_lt: Option<WebGlUniformLocation>,
    u_tint: Option<WebGlUniformLocation>,
    world: World,
    gl: G,
    prog: WebGlProgram,
    u_vp: Option<WebGlUniformLocation>,
    u_m: Option<WebGlUniformLocation>,
    u_col: Option<WebGlUniformLocation>,
    u_uvr: Option<WebGlUniformLocation>,
    canvas: HtmlCanvasElement,
    yaw: f32,
    pitch: f32,
    dist: f32,
    target: [f32; 3],
    fitted: bool,
    drag: Option<(f32, f32)>,
    status: String,
    cube: Mesh,
    sphere: Mesh,
    cyl: Mesh,
    cyl_y: Mesh,
    wedge: Mesh,
    decal_faces: Vec<Mesh>,
    ctl: Option<Controller>,
    keys: HashSet<String>,
    last_t: f64,
    send_acc: f64,
    ws: Option<WebSocket>,
    follow: bool,
    failed: HashSet<String>,
    focus: Option<[f32; 3]>,
    u_tex_on: Option<WebGlUniformLocation>,
    meshes: HashMap<String, Mesh>,
    textures: HashMap<String, Option<web_sys::WebGlTexture>>,
    requested: HashSet<String>,
    remotes: Vec<String>,
    tap: Vec<String>,
    eye: [f32; 3],
    view_fov: f32,
    queue: Vec<(String, String)>,
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

fn decal_face_data(face: usize) -> Vec<f32> {
    let g = 0.502f32;
    let q: ([f32; 3], [f32; 3], [f32; 3], [f32; 3], [f32; 3]) = match face {
        0 => ([g, 0.5, 0.5], [g, 0.5, -0.5], [g, -0.5, -0.5], [g, -0.5, 0.5], [1.0, 0.0, 0.0]),
        1 => ([0.5, g, 0.5], [-0.5, g, 0.5], [-0.5, g, -0.5], [0.5, g, -0.5], [0.0, 1.0, 0.0]),
        2 => ([-0.5, 0.5, g], [0.5, 0.5, g], [0.5, -0.5, g], [-0.5, -0.5, g], [0.0, 0.0, 1.0]),
        3 => ([-g, 0.5, -0.5], [-g, 0.5, 0.5], [-g, -0.5, 0.5], [-g, -0.5, -0.5], [-1.0, 0.0, 0.0]),
        4 => ([-0.5, -g, 0.5], [0.5, -g, 0.5], [0.5, -g, -0.5], [-0.5, -g, -0.5], [0.0, -1.0, 0.0]),
        5 => ([0.5, 0.5, -g], [-0.5, 0.5, -g], [-0.5, -0.5, -g], [0.5, -0.5, -g], [0.0, 0.0, -1.0]),
        _ => ([0.25, 0.5, -g], [-0.25, 0.5, -g], [-0.25, -0.5, -g], [0.25, -0.5, -g], [0.0, 0.0, -1.0]),
    };
    let (tl, tr, br, bl, n) = q;
    let mut v = vec![];
    for (p, uv) in [(tl, [0.0f32, 0.0]), (bl, [0.0, 1.0]), (br, [1.0, 1.0]), (tl, [0.0, 0.0]), (br, [1.0, 1.0]), (tr, [1.0, 0.0])] {
        v.extend_from_slice(&[p[0], p[1], p[2], n[0], n[1], n[2], uv[0], uv[1]]);
    }
    v
}

fn make_mesh(gl: &G, prog: &WebGlProgram, data: &[f32]) -> Mesh {
    let vao = gl.create_vertex_array();
    gl.bind_vertex_array(vao.as_ref());
    let buf = gl.create_buffer();
    gl.bind_buffer(G::ARRAY_BUFFER, buf.as_ref());
    unsafe {
        let view = js_sys_f32(data);
        gl.buffer_data_with_array_buffer_view(G::ARRAY_BUFFER, &view, G::STATIC_DRAW);
    }
    for (name, size, off) in [("p", 3, 0), ("n", 3, 12), ("uv", 2, 24)] {
        let l = gl.get_attrib_location(prog, name);
        if l >= 0 {
            gl.enable_vertex_attrib_array(l as u32);
            gl.vertex_attrib_pointer_with_i32(l as u32, size, G::FLOAT, false, 32, off);
        }
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

#[wasm_bindgen]
impl Client {
    #[wasm_bindgen(constructor)]
    pub fn new(canvas: HtmlCanvasElement) -> Result<Client, JsValue> {
        let gl: G = canvas.get_context("webgl2")?.ok_or("WebGL2 is not available")?.dyn_into()?;
        let prog = gl.create_program().unwrap();
        gl.attach_shader(&prog, &shader(&gl, G::VERTEX_SHADER, VS));
        gl.attach_shader(&prog, &shader(&gl, G::FRAGMENT_SHADER, FS));
        gl.link_program(&prog);
        let sky_prog = gl.create_program().unwrap();
        gl.attach_shader(&sky_prog, &shader(&gl, G::VERTEX_SHADER, SKY_VS));
        gl.attach_shader(&sky_prog, &shader(&gl, G::FRAGMENT_SHADER, SKY_FS));
        gl.link_program(&sky_prog);
        let sky_vao = gl.create_vertex_array();
        let u_lt = gl.get_uniform_location(&prog, "lt");
        let u_tint = gl.get_uniform_location(&prog, "tint");
        gl.use_program(Some(&prog));
        let cube_m = make_mesh(&gl, &prog, &cube());
        let sphere_m = make_mesh(&gl, &prog, &sphere_data());
        let cyl_m = make_mesh(&gl, &prog, &cylinder_data());
        let cyl_y_m = make_mesh(&gl, &prog, &cylinder_y_data());
        let wedge_m = make_mesh(&gl, &prog, &wedge_data());
        gl.enable(G::DEPTH_TEST);
        gl.enable(G::CULL_FACE);
        gl.enable(G::BLEND);
        gl.blend_func(G::SRC_ALPHA, G::ONE_MINUS_SRC_ALPHA);
        let decal_faces: Vec<Mesh> = (0..7).map(|f| make_mesh(&gl, &prog, &decal_face_data(f))).collect();
        let u_tex_on = gl.get_uniform_location(&prog, "useTex");
        let st = Rc::new(RefCell::new(State {
            sky_prog,
            sky_vao,
            u_lt,
            u_tint,
            u_vp: gl.get_uniform_location(&prog, "vp"),
            u_m: gl.get_uniform_location(&prog, "m"),
            u_col: gl.get_uniform_location(&prog, "col"),
            u_uvr: gl.get_uniform_location(&prog, "uvr"),
            world: World::new(),
            gl,
            prog,
            canvas: canvas.clone(),
            yaw: 2.4,
            pitch: 0.5,
            dist: 60.0,
            target: [0.0, 5.0, 0.0],
            fitted: false,
            drag: None,
            status: "Starting".into(),
            cube: cube_m,
            sphere: sphere_m,
            cyl: cyl_m,
            cyl_y: cyl_y_m,
            wedge: wedge_m,
            decal_faces,
            ctl: None,
            keys: HashSet::new(),
            last_t: 0.0,
            send_acc: 0.0,
            ws: None,
            follow: false,
            u_tex_on,
            failed: HashSet::new(),
            focus: None,
            meshes: HashMap::new(),
            textures: HashMap::new(),
            requested: HashSet::new(),
            remotes: vec![],
            tap: vec![],
            eye: [0.0, 0.0, 0.0],
            view_fov: 1.0,
            queue: vec![],
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
                let m = make_mesh(&st.gl, &st.prog, &data);
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
        let _ = gl.tex_image_2d_with_u32_and_u32_and_image_bitmap(G::TEXTURE_2D, 0, G::RGBA as i32, G::RGBA, G::UNSIGNED_BYTE, img);
        let (w, h) = (img.width(), img.height());
        let pot = w.is_power_of_two() && h.is_power_of_two();
        if id.starts_with("c:") {
            gl.tex_parameteri(G::TEXTURE_2D, G::TEXTURE_MIN_FILTER, G::LINEAR as i32);
            gl.tex_parameteri(G::TEXTURE_2D, G::TEXTURE_WRAP_S, G::CLAMP_TO_EDGE as i32);
            gl.tex_parameteri(G::TEXTURE_2D, G::TEXTURE_WRAP_T, G::CLAMP_TO_EDGE as i32);
        } else if pot {
            gl.generate_mipmap(G::TEXTURE_2D);
            gl.tex_parameteri(G::TEXTURE_2D, G::TEXTURE_MIN_FILTER, G::LINEAR_MIPMAP_LINEAR as i32);
            gl.tex_parameteri(G::TEXTURE_2D, G::TEXTURE_WRAP_S, G::REPEAT as i32);
            gl.tex_parameteri(G::TEXTURE_2D, G::TEXTURE_WRAP_T, G::REPEAT as i32);
        } else {
            gl.tex_parameteri(G::TEXTURE_2D, G::TEXTURE_MIN_FILTER, G::LINEAR as i32);
            gl.tex_parameteri(G::TEXTURE_2D, G::TEXTURE_WRAP_S, G::CLAMP_TO_EDGE as i32);
            gl.tex_parameteri(G::TEXTURE_2D, G::TEXTURE_WRAP_T, G::CLAMP_TO_EDGE as i32);
        }
        gl.tex_parameteri(G::TEXTURE_2D, G::TEXTURE_MAG_FILTER, G::LINEAR as i32);
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
        st.world.parts().iter().take(40).map(|p| format!("{}:[{:.1},{:.1},{:.1}] s[{:.1},{:.1},{:.1}] shape{} mesh{}", p.id, p.cf[0], p.cf[1], p.cf[2], p.size[0], p.size[1], p.size[2], p.shape, p.mesh.is_some())).collect::<Vec<_>>().join("\n")
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
        if st.world.own.is_some() {
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
        if !st.fitted {
            let (c, r) = st.world.bounds();
            st.target = c;
            st.dist = r * 1.6;
            st.fitted = true;
        }
        let eye = [
            st.target[0] + st.dist * st.pitch.cos() * st.yaw.sin(),
            st.target[1] + st.dist * st.pitch.sin(),
            st.target[2] + st.dist * st.pitch.cos() * st.yaw.cos(),
        ];
        st.eye = eye;
        let proj = mat::perspective(1.0, w as f32 / h as f32, 0.5, 20000.0);
        let view = mat::look_at(eye, st.target, [0.0, 1.0, 0.0]);
        let vp = mat::mul(&proj, &view);
        let faces = st.world.sky_faces();
        let mut sky_ready = true;
        let mut wants_sky: Vec<(String, String)> = vec![];
        for f in faces.iter() {
            match st.textures.get(f) {
                Some(Some(_)) => {}
                Some(None) => {}
                None => {
                    sky_ready = false;
                    wants_sky.push(("tex".into(), f.clone()));
                }
            }
        }
        for (k, id) in wants_sky {
            let key = format!("{}:{}", k, id);
            if st.requested.insert(key) {
                st.queue.push((k, id));
            }
        }
        if sky_ready {
            let d = [target_minus(st.target, eye)];
            let dirv = norm3(d[0]);
            let up_ref = if dirv[1].abs() > 0.99 { [0.0, 0.0, 1.0] } else { [0.0, 1.0, 0.0] };
            let rightv = norm3(cross3(dirv, up_ref));
            let upv = cross3(rightv, dirv);
            let tan_y = (0.5f32).tan();
            gl.use_program(Some(&st.sky_prog));
            gl.depth_mask(false);
            gl.disable(G::DEPTH_TEST);
            gl.disable(G::CULL_FACE);
            for (i, f) in faces.iter().enumerate() {
                gl.active_texture(G::TEXTURE1 + i as u32);
                if let Some(Some(t)) = st.textures.get(f) {
                    gl.bind_texture(G::TEXTURE_2D, Some(t));
                }
                gl.uniform1i(gl.get_uniform_location(&st.sky_prog, &format!("f{}", i)).as_ref(), 1 + i as i32);
            }
            gl.uniform3f(gl.get_uniform_location(&st.sky_prog, "dir").as_ref(), dirv[0], dirv[1], dirv[2]);
            gl.uniform3f(gl.get_uniform_location(&st.sky_prog, "right").as_ref(), rightv[0], rightv[1], rightv[2]);
            gl.uniform3f(gl.get_uniform_location(&st.sky_prog, "up").as_ref(), upv[0], upv[1], upv[2]);
            gl.uniform2f(gl.get_uniform_location(&st.sky_prog, "tanv").as_ref(), tan_y * (w as f32 / h as f32), tan_y);
            gl.bind_vertex_array(st.sky_vao.as_ref());
            gl.draw_arrays(G::TRIANGLES, 0, 3);
            gl.enable(G::DEPTH_TEST);
            gl.enable(G::CULL_FACE);
            gl.depth_mask(true);
            gl.active_texture(G::TEXTURE0);
        }
        gl.use_program(Some(&st.prog));
        gl.uniform4f(st.u_uvr.as_ref(), 0.0, 0.0, 1.0, 1.0);
        let lt = st.world.light();
        gl.uniform4f(st.u_lt.as_ref(), lt.amb, lt.key, lt.fill, 0.0);
        gl.uniform3f(st.u_tint.as_ref(), lt.tint[0], lt.tint[1], lt.tint[2]);
        gl.uniform_matrix4fv_with_f32_array(st.u_vp.as_ref(), false, &vp);
        let parts = st.world.parts();
        let mut solid: Vec<&swirl_net::world::Part> = parts.iter().filter(|p| p.alpha >= 0.99).collect();
        let mut glass: Vec<&swirl_net::world::Part> = parts.iter().filter(|p| p.alpha < 0.99 && p.alpha > 0.01).collect();
        glass.sort_by(|a, b| {
            let da = (a.cf[0] - eye[0]).powi(2) + (a.cf[1] - eye[1]).powi(2) + (a.cf[2] - eye[2]).powi(2);
            let db = (b.cf[0] - eye[0]).powi(2) + (b.cf[1] - eye[1]).powi(2) + (b.cf[2] - eye[2]).powi(2);
            db.partial_cmp(&da).unwrap_or(std::cmp::Ordering::Equal)
        });
        solid.extend(glass);
        let mut wants: Vec<(String, String)> = vec![];
        for p in solid {
            let base = mat::from_cframe(&p.cf, [1.0, 1.0, 1.0]);
            let mut drawn = false;
            if let Some(mr) = &p.mesh {
                let key = mr.mesh.clone();
                if let Some(mesh) = st.meshes.get(&key) {
                    let (s, o) = if mr.fit {
                        let ext = [(mesh.hi[0] - mesh.lo[0]).max(0.001), (mesh.hi[1] - mesh.lo[1]).max(0.001), (mesh.hi[2] - mesh.lo[2]).max(0.001)];
                        let s = [p.size[0] / ext[0], p.size[1] / ext[1], p.size[2] / ext[2]];
                        let c = [(mesh.hi[0] + mesh.lo[0]) / 2.0, (mesh.hi[1] + mesh.lo[1]) / 2.0, (mesh.hi[2] + mesh.lo[2]) / 2.0];
                        (s, [-c[0] * s[0], -c[1] * s[1], -c[2] * s[2]])
                    } else {
                        (mr.scale, mr.off)
                    };
                    let local = [s[0], 0.0, 0.0, 0.0, 0.0, s[1], 0.0, 0.0, 0.0, 0.0, s[2], 0.0, o[0], o[1], o[2], 1.0];
                    let m = mat::mul(&base, &local);
                    gl.uniform_matrix4fv_with_f32_array(st.u_m.as_ref(), false, &m);
                    gl.uniform4f(st.u_col.as_ref(), p.color[0], p.color[1], p.color[2], p.alpha);
                    let mut tex_on = 0.0;
                    if let Some(tid) = &mr.tex {
                        match st.textures.get(tid) {
                            Some(Some(t)) => {
                                gl.active_texture(G::TEXTURE0);
                                gl.bind_texture(G::TEXTURE_2D, Some(t));
                                tex_on = 1.0;
                            }
                            Some(None) => {}
                            None => wants.push(("tex".into(), tid.clone())),
                        }
                    }
                    gl.uniform1f(st.u_tex_on.as_ref(), tex_on);
                    gl.disable(G::CULL_FACE);
                    gl.bind_vertex_array(mesh.vao.as_ref());
                    gl.draw_arrays(G::TRIANGLES, 0, mesh.count);
                    gl.enable(G::CULL_FACE);
                    drawn = true;
                } else if !st.failed.contains(&key) {
                    wants.push(("mesh".into(), key));
                    drawn = true;
                }
            }
            if drawn {
                continue;
            }
            let m = mat::from_cframe(&p.cf, p.size);
            gl.uniform_matrix4fv_with_f32_array(st.u_m.as_ref(), false, &m);
            gl.uniform4f(st.u_col.as_ref(), p.color[0], p.color[1], p.color[2], p.alpha);
            gl.uniform1f(st.u_tex_on.as_ref(), 0.0);
            let mesh = match p.shape {
                0 => &st.sphere,
                2 => &st.cyl,
                3 => &st.wedge,
                4 => &st.cyl_y,
                _ => &st.cube,
            };
            gl.bind_vertex_array(mesh.vao.as_ref());
            gl.draw_arrays(G::TRIANGLES, 0, mesh.count);
        }
        gl.depth_mask(false);
        gl.uniform4f(st.u_lt.as_ref(), 1.0, 0.0, 0.0, 0.0);
        gl.uniform3f(st.u_tint.as_ref(), 1.0, 1.0, 1.0);
        for p in parts.iter().filter(|p| !p.decals.is_empty()) {
            let m = mat::from_cframe(&p.cf, p.size);
            gl.uniform_matrix4fv_with_f32_array(st.u_m.as_ref(), false, &m);
            for (face, tid, da, rect) in &p.decals {
                match st.textures.get(tid) {
                    Some(Some(t)) => {
                        gl.active_texture(G::TEXTURE0);
                        gl.bind_texture(G::TEXTURE_2D, Some(t));
                        gl.uniform1f(st.u_tex_on.as_ref(), 2.0);
                        gl.uniform4f(st.u_col.as_ref(), 1.0, 1.0, 1.0, *da);
                        gl.uniform4f(st.u_uvr.as_ref(), rect[0], rect[1], rect[2], rect[3]);
                        let mesh = &st.decal_faces[(*face as usize).min(6)];
                        gl.bind_vertex_array(mesh.vao.as_ref());
                        gl.draw_arrays(G::TRIANGLES, 0, mesh.count);
                    }
                    Some(None) => {}
                    None => wants.push(("tex".into(), tid.clone())),
                }
            }
        }
        gl.uniform4f(st.u_uvr.as_ref(), 0.0, 0.0, 1.0, 1.0);
        gl.depth_mask(true);
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
