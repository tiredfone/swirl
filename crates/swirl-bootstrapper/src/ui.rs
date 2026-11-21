use crate::draw::{self, Assets, Btn, View};
use crate::era::{Era, Phase};
use crate::flow::{self, Ctx, Outcome, Sh, Shared};
use crate::paths::Paths;
use minifb::{MouseButton, MouseMode, Window, WindowOptions};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

struct Args {
    screenshot: Option<String>,
    state: Option<String>,
    headless: bool,
    channel: Option<String>,
    rest: Vec<String>,
}

fn parse_args() -> Args {
    let mut a = Args { screenshot: None, state: None, headless: std::env::var_os("SOLAR_HEADLESS").is_some(), channel: None, rest: Vec::new() };
    let mut it = std::env::args().skip(1);
    while let Some(x) = it.next() {
        match x.as_str() {
            "--screenshot" => a.screenshot = it.next(),
            "--state" => a.state = it.next(),
            "--headless" => a.headless = true,
            _ => {
                if let Some(c) = x.strip_prefix("--channel=") {
                    a.channel = Some(c.to_string());
                } else {
                    a.rest.push(x);
                }
            }
        }
    }
    a
}

fn view_from(sh: &Shared, tick: f32, hover: Option<Btn>) -> View {
    View { phase: sh.phase, percent: sh.percent, error: sh.error.clone(), installed_only: sh.installed_only, tick, hover }
}

fn write_png(path: &str, c: &draw::Canvas) -> Result<(), String> {
    let f = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(f), c.w as u32, c.h as u32);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    let mut w = enc.write_header().map_err(|e| e.to_string())?;
    let mut data = Vec::with_capacity(c.w * c.h * 3);
    for p in &c.buf {
        data.extend_from_slice(&[(p >> 16) as u8, (p >> 8) as u8, *p as u8]);
    }
    w.write_image_data(&data).map_err(|e| e.to_string())
}

fn screenshot(era: Era, path: &str, state: &str) {
    let (name, arg) = state.split_once(':').map(|(a, b)| (a, Some(b))).unwrap_or((state, None));
    let mut sh = Shared::new();
    match Phase::parse(name) {
        Some(Phase::Error) => {
            sh.phase = Phase::Error;
            sh.error = Some(arg.unwrap_or("Cannot reach the server (Dns)").to_string());
        }
        Some(Phase::Done) => {
            sh.phase = Phase::Done;
            sh.installed_only = arg == Some("installed");
        }
        Some(p) => {
            sh.phase = p;
            sh.percent = arg.and_then(|a| a.parse().ok());
        }
        None => {
            eprintln!("unknown state {}", name);
            std::process::exit(2);
        }
    }
    let assets = Assets::load();
    let v = view_from(&sh, 0.55, None);
    if let Err(e) = write_png(path, &draw::render(era, &assets, &v)) {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}

fn worker(sh: Sh, ctx: Arc<Ctx>) -> i32 {
    loop {
        {
            let mut s = sh.lock().unwrap();
            s.error = None;
            s.retry = false;
            s.installed_only = false;
        }
        match flow::run_once(&sh, &ctx) {
            Ok(Outcome::Installed) => {
                if !ctx.headless {
                    std::thread::sleep(Duration::from_millis(2200));
                }
                sh.lock().unwrap().finished = true;
                return 0;
            }
            Ok(_) => {
                sh.lock().unwrap().finished = true;
                return 0;
            }
            Err(e) => {
                flow::log(&ctx, &format!("error: {}", e));
                if sh.lock().unwrap().cancel {
                    sh.lock().unwrap().finished = true;
                    return 0;
                }
                if ctx.headless {
                    sh.lock().unwrap().finished = true;
                    return 1;
                }
                {
                    let mut s = sh.lock().unwrap();
                    s.phase = Phase::Error;
                    s.error = Some(e);
                }
                loop {
                    std::thread::sleep(Duration::from_millis(50));
                    let s = sh.lock().unwrap();
                    if s.cancel {
                        drop(s);
                        sh.lock().unwrap().finished = true;
                        return 0;
                    }
                    if s.retry {
                        break;
                    }
                }
            }
        }
    }
}

pub fn main(era: Era) {
    let args = parse_args();
    if let Some(p) = &args.screenshot {
        screenshot(era, p, args.state.as_deref().unwrap_or("connecting"));
        return;
    }
    let launch = match flow::parse_launch(era, &args.rest) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("{}", e);
            None
        }
    };
    let ctx = Arc::new(Ctx {
        era,
        paths: Paths::new(era),
        channel: args.channel.clone().or_else(|| launch.as_ref().map(|l| l.channel.clone())).unwrap_or_else(|| "live".into()),
        launch,
        raw_args: args.rest.clone(),
        headless: args.headless,
    });
    let sh: Sh = Arc::new(Mutex::new(Shared::new()));
    if args.headless {
        let code = worker(sh, ctx);
        std::process::exit(code);
    }
    let (w, h) = era.window_size();
    let title = if era == Era::E2012 { "Solar 2012" } else { "Solar" };
    let win = Window::new(title, w, h, WindowOptions::default());
    let mut win = match win {
        Ok(w) => w,
        Err(e) => {
            eprintln!("No window available ({}), running without one", e);
            let mut c2 = Ctx { era, paths: Paths::new(era), channel: ctx.channel.clone(), launch: ctx.launch.clone(), raw_args: ctx.raw_args.clone(), headless: true };
            c2.headless = true;
            std::process::exit(worker(sh, Arc::new(c2)));
        }
    };
    win.set_target_fps(30);
    let (sh2, ctx2) = (sh.clone(), ctx.clone());
    let handle = std::thread::spawn(move || worker(sh2, ctx2));
    let assets = Assets::load();
    let start = Instant::now();
    let mut was_down = false;
    loop {
        if !win.is_open() {
            sh.lock().unwrap().cancel = true;
            std::thread::sleep(Duration::from_millis(100));
            break;
        }
        let snap = {
            let s = sh.lock().unwrap();
            if s.finished {
                break;
            }
            view_from(&s, start.elapsed().as_secs_f32(), None)
        };
        let mouse = win.get_mouse_pos(MouseMode::Discard);
        let btns = draw::buttons(era, &snap);
        let hover = mouse.and_then(|(x, y)| btns.iter().find(|(_, r)| r.contains(x, y)).map(|(b, _)| *b));
        let down = win.get_mouse_down(MouseButton::Left);
        if down && !was_down {
            match hover {
                Some(Btn::Cancel) | Some(Btn::Close) => {
                    sh.lock().unwrap().cancel = true;
                    std::thread::sleep(Duration::from_millis(150));
                    break;
                }
                Some(Btn::Retry) => sh.lock().unwrap().retry = true,
                None => {}
            }
        }
        was_down = down;
        let v = View { hover, ..snap };
        let canvas = draw::render(era, &assets, &v);
        if win.update_with_buffer(&canvas.buf, w, h).is_err() {
            break;
        }
    }
    let _ = handle.is_finished();
    std::process::exit(0);
}
