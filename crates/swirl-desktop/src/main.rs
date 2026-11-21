#![cfg_attr(windows, windows_subsystem = "windows")]

mod game;
mod gfx;
mod net;
mod shapes;

use game::Game;
use glutin::config::{ConfigTemplateBuilder, GlConfig};
use glutin::context::{ContextApi, ContextAttributesBuilder, NotCurrentGlContext, PossiblyCurrentContext, Version};
use glutin::display::{GetGlDisplay, GlDisplay};
use glutin::surface::{GlSurface, Surface, SwapInterval, WindowSurface};
use glutin_winit::{DisplayBuilder, GlWindow};
use raw_window_handle::HasWindowHandle;
use std::io::Write;
use std::num::NonZeroU32;
use std::sync::mpsc::{Receiver, Sender};
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

struct Opts {
    place: String,
    token: String,
    base: String,
    direct: Option<String>,
    screenshot: Option<String>,
    frames: u32,
    offscreen: bool,
}

fn say(line: &str) {
    let mut o = std::io::stdout();
    let _ = writeln!(o, "{}", line);
    let _ = o.flush();
}

fn fail(msg: &str) -> ! {
    say(&format!("SOLAR_ERROR {}", msg));
    std::process::exit(1);
}

fn parse() -> Opts {
    let mut o = Opts { place: String::new(), token: String::new(), base: "https://imfone.dev".into(), direct: None, screenshot: None, frames: 20, offscreen: false };
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--place" => o.place = it.next().unwrap_or_default(),
            "--token" => o.token = it.next().unwrap_or_default(),
            "--base-url" => o.base = it.next().unwrap_or_default(),
            "--direct" => o.direct = it.next(),
            "--screenshot" => o.screenshot = it.next(),
            "--offscreen" => o.offscreen = true,
            "--frames" => o.frames = it.next().and_then(|v| v.parse().ok()).unwrap_or(20),
            "--channel" | "--server" => {
                let _ = it.next();
            }
            _ => {}
        }
    }
    o
}

struct Gl {
    window: Window,
    surface: Surface<WindowSurface>,
    ctx: PossiblyCurrentContext,
    gfx: gfx::Gfx,
}

struct App {
    opts: Opts,
    game: Game,
    rx: Receiver<net::Msg>,
    tx: Sender<String>,
    gl: Option<Gl>,
    last: Instant,
    ready_said: bool,
    drag: Option<(f64, f64)>,
    cursor: (f64, f64),
    frames_ready: u32,
    started: Instant,
}

impl App {
    fn make(&mut self, el: &ActiveEventLoop) -> Result<(), String> {
        let attrs = Window::default_attributes().with_title("Solar").with_inner_size(LogicalSize::new(1024.0, 640.0));
        let builder = DisplayBuilder::new().with_window_attributes(Some(attrs));
        let (window, config) = builder
            .build(el, ConfigTemplateBuilder::new().with_depth_size(24), |c| c.reduce(|a, b| if b.num_samples() > a.num_samples() { b } else { a }).unwrap())
            .map_err(|e| format!("No graphics available ({})", e))?;
        let window = window.ok_or("No window")?;
        let raw = window.window_handle().map_err(|e| e.to_string())?.as_raw();
        let display = config.display();
        let ctx_attrs = ContextAttributesBuilder::new().with_context_api(ContextApi::OpenGl(Some(Version::new(3, 3)))).build(Some(raw));
        let not_current = unsafe { display.create_context(&config, &ctx_attrs) }.map_err(|e| format!("OpenGL 3.3 is not available ({})", e))?;
        let sattrs = window.build_surface_attributes(Default::default()).map_err(|e| e.to_string())?;
        let surface = unsafe { display.create_window_surface(&config, &sattrs) }.map_err(|e| e.to_string())?;
        let ctx = not_current.make_current(&surface).map_err(|e| e.to_string())?;
        let _ = surface.set_swap_interval(&ctx, SwapInterval::Wait(NonZeroU32::new(1).unwrap()));
        let glc = unsafe { glow::Context::from_loader_function_cstr(|s| display.get_proc_address(s)) };
        let gfx = gfx::Gfx::new(glc)?;
        self.gl = Some(Gl { window, surface, ctx, gfx });
        Ok(())
    }

    fn frame(&mut self, el: &ActiveEventLoop) {
        while let Ok(m) = self.rx.try_recv() {
            match m {
                net::Msg::Text(t) => self.game.apply(&t),
                net::Msg::Closed(r) => fail(&r),
            }
        }
        let now = Instant::now();
        let dt = now.duration_since(self.last).as_secs_f32().clamp(0.0, 0.1);
        self.last = now;
        let Some(g) = self.gl.as_ref() else { return };
        let size = g.window.inner_size();
        let (w, h) = (size.width.max(1), size.height.max(1));
        g.surface.resize(&g.ctx, NonZeroU32::new(w).unwrap(), NonZeroU32::new(h).unwrap());
        let f = self.game.step(dt, w as f32 / h as f32);
        for m in self.game.out.drain(..) {
            let _ = self.tx.send(m);
        }
        g.gfx.draw(w, h, f.as_ref());
        if f.is_some() {
            self.frames_ready += 1;
            if let Some(path) = &self.opts.screenshot {
                if self.frames_ready >= self.opts.frames {
                    let px = g.gfx.read_rgba(w, h);
                    write_png(path, w, h, &px);
                    say("SOLAR_READY");
                    el.exit();
                    return;
                }
            }
        }
        let _ = g.surface.swap_buffers(&g.ctx);
        if f.is_some() && !self.ready_said {
            self.ready_said = true;
            say("SOLAR_READY");
        }
        if self.opts.screenshot.is_some() && self.started.elapsed().as_secs() > 40 {
            fail("Timed out waiting for the place");
        }
        g.window.set_title(&format!("Solar - {} parts", self.game.world.parts().len()));
        g.window.request_redraw();
    }
}

fn write_png(path: &str, w: u32, h: u32, rgba: &[u8]) {
    let mut flipped = Vec::with_capacity(rgba.len());
    for row in (0..h as usize).rev() {
        flipped.extend_from_slice(&rgba[row * w as usize * 4..(row + 1) * w as usize * 4]);
    }
    if let Ok(f) = std::fs::File::create(path) {
        let mut enc = png::Encoder::new(std::io::BufWriter::new(f), w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        if let Ok(mut wr) = enc.write_header() {
            let _ = wr.write_image_data(&flipped);
        }
    }
}

fn key_name(k: &Key) -> Option<String> {
    match k {
        Key::Character(c) => Some(c.to_lowercase()),
        Key::Named(NamedKey::Space) => Some(" ".into()),
        Key::Named(NamedKey::ArrowUp) => Some("arrowup".into()),
        Key::Named(NamedKey::ArrowDown) => Some("arrowdown".into()),
        Key::Named(NamedKey::ArrowLeft) => Some("arrowleft".into()),
        Key::Named(NamedKey::ArrowRight) => Some("arrowright".into()),
        _ => None,
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.gl.is_none() {
            if let Err(e) = self.make(el) {
                fail(&e);
            }
            if let Some(g) = &self.gl {
                g.window.request_redraw();
            }
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, ev: WindowEvent) {
        match ev {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::RedrawRequested => self.frame(el),
            WindowEvent::KeyboardInput { event, .. } => {
                if let Some(k) = key_name(&event.logical_key) {
                    if event.state == ElementState::Pressed {
                        self.game.keys.insert(k);
                    } else {
                        self.game.keys.remove(&k);
                    }
                }
            }
            WindowEvent::MouseInput { state, button: MouseButton::Left | MouseButton::Right, .. } => {
                self.drag = if state == ElementState::Pressed { Some(self.cursor) } else { None };
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = (position.x, position.y);
                if let Some((x, y)) = self.drag {
                    self.game.yaw -= (position.x - x) as f32 * 0.008;
                    self.game.pitch = (self.game.pitch + (position.y - y) as f32 * 0.008).clamp(-1.5, 1.5);
                    self.drag = Some((position.x, position.y));
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let d = match delta {
                    MouseScrollDelta::LineDelta(_, y) => -y,
                    MouseScrollDelta::PixelDelta(p) => -(p.y as f32) / 40.0,
                };
                self.game.dist = (self.game.dist * (1.0 + d.signum() * 0.1)).clamp(3.0, 2000.0);
            }
            _ => {}
        }
    }
}

#[cfg(unix)]
fn offscreen(o: &Opts, rx: Receiver<net::Msg>, tx: Sender<String>) {
    use glutin::config::ConfigSurfaceTypes;
    use glutin::context::NotCurrentGlContext as _;
    use glutin::display::{Display, DisplayApiPreference};
    use glutin::surface::{PbufferSurface, SurfaceAttributesBuilder};
    use raw_window_handle::{RawDisplayHandle, XlibDisplayHandle};
    let (w, h) = (1024u32, 640u32);
    let display = unsafe { Display::new(RawDisplayHandle::Xlib(XlibDisplayHandle::new(None, 0)), DisplayApiPreference::Egl) }.unwrap_or_else(|e| fail(&format!("No display ({})", e)));
    let template = ConfigTemplateBuilder::new().with_surface_type(ConfigSurfaceTypes::PBUFFER).with_depth_size(24).build();
    let config = unsafe { display.find_configs(template) }.ok().and_then(|mut c| c.next()).unwrap_or_else(|| fail("No graphics configuration"));
    let attrs = ContextAttributesBuilder::new().with_context_api(ContextApi::OpenGl(Some(Version::new(3, 3)))).build(None);
    let not_current = unsafe { display.create_context(&config, &attrs) }.unwrap_or_else(|e| fail(&format!("OpenGL 3.3 is not available ({})", e)));
    let sattrs = SurfaceAttributesBuilder::<PbufferSurface>::new().build(NonZeroU32::new(w).unwrap(), NonZeroU32::new(h).unwrap());
    let surface = unsafe { display.create_pbuffer_surface(&config, &sattrs) }.unwrap_or_else(|e| fail(&format!("{}", e)));
    let _ctx = not_current.make_current(&surface).unwrap_or_else(|e| fail(&format!("{}", e)));
    let glc = unsafe { glow::Context::from_loader_function_cstr(|s| display.get_proc_address(s)) };
    let gfx = gfx::Gfx::new(glc).unwrap_or_else(|e| fail(&e));
    let mut game = Game::new();
    let mut last = Instant::now();
    let started = Instant::now();
    let mut ready_frames = 0;
    loop {
        while let Ok(m) = rx.try_recv() {
            match m {
                net::Msg::Text(t) => game.apply(&t),
                net::Msg::Closed(r) => fail(&r),
            }
        }
        let now = Instant::now();
        let dt = now.duration_since(last).as_secs_f32().clamp(0.0, 0.1);
        last = now;
        let f = game.step(dt, w as f32 / h as f32);
        for m in game.out.drain(..) {
            let _ = tx.send(m);
        }
        gfx.draw(w, h, f.as_ref());
        if f.is_some() {
            ready_frames += 1;
            if ready_frames >= o.frames {
                let px = gfx.read_rgba(w, h);
                write_png(o.screenshot.as_deref().unwrap_or("offscreen.png"), w, h, &px);
                say(&format!("SOLAR_READY parts={}", game.world.parts().len()));
                std::process::exit(0);
            }
        }
        if started.elapsed().as_secs() > 40 {
            fail("Timed out waiting for the place");
        }
        std::thread::sleep(std::time::Duration::from_millis(16));
    }
}

fn main() {
    let o = parse();
    let url = match &o.direct {
        Some(u) => u.clone(),
        None => {
            if o.token.is_empty() {
                fail("No launch token");
            }
            match net::redeem(&o.base, &o.token) {
                Ok(c) => {
                    let _ = (c.era, c.place, &c.user);
                    net::with_ticket(&c.ws, &c.ticket)
                }
                Err(e) => fail(&e),
            }
        }
    };
    let (rx, tx) = net::connect(url);
    #[cfg(unix)]
    if o.offscreen {
        offscreen(&o, rx, tx);
        return;
    }
    let el = match EventLoop::new() {
        Ok(e) => e,
        Err(e) => fail(&format!("No window system ({})", e)),
    };
    el.set_control_flow(ControlFlow::Poll);
    let mut app = App { opts: o, game: Game::new(), rx, tx, gl: None, last: Instant::now(), ready_said: false, drag: None, cursor: (0.0, 0.0), frames_ready: 0, started: Instant::now() };
    let _ = el.run_app(&mut app);
}
