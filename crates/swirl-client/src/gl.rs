use std::cell::RefCell;
use std::rc::Rc;
use swirl_net::mat;
use swirl_net::world::World;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{HtmlCanvasElement, WebGl2RenderingContext as G, WebGlProgram, WebGlUniformLocation, WebSocket};

const VS: &str = "#version 300 es
in vec3 p; in vec3 n;
uniform mat4 vp; uniform mat4 m;
out vec3 vn;
void main(){ vn = mat3(m) * n; gl_Position = vp * m * vec4(p,1.0); }";
const FS: &str = "#version 300 es
precision mediump float;
in vec3 vn; uniform vec4 col; out vec4 o;
void main(){ vec3 N = normalize(vn); float d = max(dot(N, normalize(vec3(0.47,0.82,-0.31))), 0.0); float f = max(dot(N, normalize(vec3(-0.5,-0.2,0.6))), 0.0);
float l = 0.5 + 0.6*d + 0.2*f; o = vec4(col.rgb * l, col.a); }";

struct State {
    world: World,
    gl: G,
    prog: WebGlProgram,
    u_vp: Option<WebGlUniformLocation>,
    u_m: Option<WebGlUniformLocation>,
    u_col: Option<WebGlUniformLocation>,
    canvas: HtmlCanvasElement,
    yaw: f32,
    pitch: f32,
    dist: f32,
    target: [f32; 3],
    fitted: bool,
    drag: Option<(f32, f32)>,
    status: String,
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
    for (n, q) in faces.iter() {
        for i in [0usize, 1, 2, 0, 2, 3] {
            v.extend_from_slice(&q[i]);
            v.extend_from_slice(n);
        }
    }
    v
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
        gl.use_program(Some(&prog));
        let vao = gl.create_vertex_array();
        gl.bind_vertex_array(vao.as_ref());
        let buf = gl.create_buffer();
        gl.bind_buffer(G::ARRAY_BUFFER, buf.as_ref());
        let data = cube();
        unsafe {
            let view = js_sys_f32(&data);
            gl.buffer_data_with_array_buffer_view(G::ARRAY_BUFFER, &view, G::STATIC_DRAW);
        }
        let (lp, ln) = (gl.get_attrib_location(&prog, "p") as u32, gl.get_attrib_location(&prog, "n") as u32);
        gl.enable_vertex_attrib_array(lp);
        gl.vertex_attrib_pointer_with_i32(lp, 3, G::FLOAT, false, 24, 0);
        gl.enable_vertex_attrib_array(ln);
        gl.vertex_attrib_pointer_with_i32(ln, 3, G::FLOAT, false, 24, 12);
        gl.enable(G::DEPTH_TEST);
        gl.enable(G::CULL_FACE);
        gl.enable(G::BLEND);
        gl.blend_func(G::SRC_ALPHA, G::ONE_MINUS_SRC_ALPHA);
        let st = Rc::new(RefCell::new(State {
            u_vp: gl.get_uniform_location(&prog, "vp"),
            u_m: gl.get_uniform_location(&prog, "m"),
            u_col: gl.get_uniform_location(&prog, "col"),
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
        Ok(Client { st, _ws: None })
    }

    pub fn connect(&mut self, url: &str) -> Result<(), JsValue> {
        let ws = WebSocket::new(url)?;
        let s = self.st.clone();
        let on = Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |e: web_sys::MessageEvent| {
            if let Some(t) = e.data().as_string() {
                let mut st = s.borrow_mut();
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
        self._ws = Some(ws);
        Ok(())
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

    pub fn frame(&self) {
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
        let proj = mat::perspective(1.0, w as f32 / h as f32, 0.5, 20000.0);
        let view = mat::look_at(eye, st.target, [0.0, 1.0, 0.0]);
        let vp = mat::mul(&proj, &view);
        gl.use_program(Some(&st.prog));
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
        for p in solid {
            let m = mat::from_cframe(&p.cf, p.size);
            gl.uniform_matrix4fv_with_f32_array(st.u_m.as_ref(), false, &m);
            gl.uniform4f(st.u_col.as_ref(), p.color[0], p.color[1], p.color[2], p.alpha);
            gl.draw_arrays(G::TRIANGLES, 0, 36);
        }
    }
}

unsafe fn js_sys_f32(data: &[f32]) -> web_sys::js_sys::Float32Array {
    web_sys::js_sys::Float32Array::view(data)
}
