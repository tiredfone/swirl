use crate::game::Frame;
use crate::shapes;
use glow::HasContext;
use swirl_net::mat;

const VS: &str = "#version 330 core
in vec3 p; in vec3 n;
uniform mat4 vp; uniform mat4 m;
out vec3 vn;
void main(){ vn = mat3(m) * n; gl_Position = vp * m * vec4(p,1.0); }";
const FS: &str = "#version 330 core
in vec3 vn; uniform vec4 col; out vec4 o;
void main(){ vec3 N = normalize(vn); float d = max(dot(N, normalize(vec3(0.47,0.82,-0.31))), 0.0); float f = max(dot(N, normalize(vec3(-0.5,-0.2,0.6))), 0.0);
float l = 0.5 + 0.6*d + 0.2*f; o = vec4(col.rgb * l, col.a); }";

struct Mesh {
    vao: glow::VertexArray,
    count: i32,
}

pub struct Gfx {
    gl: glow::Context,
    prog: glow::Program,
    u_vp: Option<glow::UniformLocation>,
    u_m: Option<glow::UniformLocation>,
    u_col: Option<glow::UniformLocation>,
    meshes: [Mesh; 3],
}

unsafe fn compile(gl: &glow::Context, kind: u32, src: &str) -> Result<glow::Shader, String> {
    let s = gl.create_shader(kind)?;
    gl.shader_source(s, src);
    gl.compile_shader(s);
    if !gl.get_shader_compile_status(s) {
        return Err(gl.get_shader_info_log(s));
    }
    Ok(s)
}

impl Gfx {
    pub fn new(gl: glow::Context) -> Result<Gfx, String> {
        unsafe {
            let prog = gl.create_program()?;
            let vs = compile(&gl, glow::VERTEX_SHADER, VS)?;
            let fs = compile(&gl, glow::FRAGMENT_SHADER, FS)?;
            gl.attach_shader(prog, vs);
            gl.attach_shader(prog, fs);
            gl.link_program(prog);
            if !gl.get_program_link_status(prog) {
                return Err(gl.get_program_info_log(prog));
            }
            gl.use_program(Some(prog));
            let make = |data: Vec<f32>| -> Result<Mesh, String> {
                let vao = gl.create_vertex_array()?;
                gl.bind_vertex_array(Some(vao));
                let buf = gl.create_buffer()?;
                gl.bind_buffer(glow::ARRAY_BUFFER, Some(buf));
                let bytes = std::slice::from_raw_parts(data.as_ptr() as *const u8, data.len() * 4);
                gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytes, glow::STATIC_DRAW);
                let lp = gl.get_attrib_location(prog, "p").ok_or("no p")?;
                let ln = gl.get_attrib_location(prog, "n").ok_or("no n")?;
                gl.enable_vertex_attrib_array(lp);
                gl.vertex_attrib_pointer_f32(lp, 3, glow::FLOAT, false, 24, 0);
                gl.enable_vertex_attrib_array(ln);
                gl.vertex_attrib_pointer_f32(ln, 3, glow::FLOAT, false, 24, 12);
                Ok(Mesh { vao, count: (data.len() / 6) as i32 })
            };
            let meshes = [make(shapes::sphere())?, make(shapes::cube())?, make(shapes::cylinder())?];
            gl.enable(glow::DEPTH_TEST);
            gl.enable(glow::CULL_FACE);
            gl.enable(glow::BLEND);
            gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
            Ok(Gfx {
                u_vp: gl.get_uniform_location(prog, "vp"),
                u_m: gl.get_uniform_location(prog, "m"),
                u_col: gl.get_uniform_location(prog, "col"),
                gl,
                prog,
                meshes,
            })
        }
    }

    pub fn draw(&self, w: u32, h: u32, frame: Option<&Frame>) {
        let gl = &self.gl;
        unsafe {
            gl.viewport(0, 0, w as i32, h as i32);
            gl.clear_color(0.55, 0.72, 0.93, 1.0);
            gl.clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);
            let f = match frame {
                Some(f) => f,
                None => return,
            };
            gl.use_program(Some(self.prog));
            gl.uniform_matrix_4_f32_slice(self.u_vp.as_ref(), false, &f.vp);
            for p in &f.parts {
                let m = mat::from_cframe(&p.cf, p.size);
                gl.uniform_matrix_4_f32_slice(self.u_m.as_ref(), false, &m);
                gl.uniform_4_f32(self.u_col.as_ref(), p.color[0], p.color[1], p.color[2], p.alpha);
                let mesh = match p.shape {
                    0 => &self.meshes[0],
                    2 => &self.meshes[2],
                    _ => &self.meshes[1],
                };
                gl.bind_vertex_array(Some(mesh.vao));
                gl.draw_arrays(glow::TRIANGLES, 0, mesh.count);
            }
        }
    }

    pub fn read_rgba(&self, w: u32, h: u32) -> Vec<u8> {
        let mut px = vec![0u8; (w * h * 4) as usize];
        unsafe {
            self.gl.read_pixels(0, 0, w as i32, h as i32, glow::RGBA, glow::UNSIGNED_BYTE, glow::PixelPackData::Slice(Some(&mut px)));
        }
        px
    }
}
