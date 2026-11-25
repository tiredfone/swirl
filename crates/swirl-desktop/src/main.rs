#![cfg_attr(windows, windows_subsystem = "windows")]

use serde_json::{json, Value};
use std::io::Write;
use std::time::Duration;
use tao::dpi::LogicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::window::WindowBuilder;
use wry::WebViewBuilder;

struct Opts {
    token: String,
    base: String,
    channel: String,
    title: String,
}

fn say(line: &str) {
    let mut o = std::io::stdout();
    let _ = writeln!(o, "{}", line);
    let _ = o.flush();
}

fn fail(msg: &str) -> ! {
    say(&format!("SOLAR_ERROR {}", msg.replace('\n', " ")));
    std::process::exit(1);
}

fn parse() -> Opts {
    let mut o = Opts { token: String::new(), base: "https://imfone.dev".into(), channel: "developer".into(), title: "Solar".into() };
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--token" => o.token = it.next().unwrap_or_default(),
            "--base-url" => o.base = it.next().unwrap_or_default(),
            "--channel" => o.channel = it.next().unwrap_or_default(),
            "--title" => o.title = it.next().unwrap_or_default(),
            "--place" | "--server" => {
                let _ = it.next();
            }
            _ => {}
        }
    }
    o.base = o.base.trim_end_matches('/').to_string();
    o
}

fn redeem(base: &str, token: &str) -> Result<Value, String> {
    let url = format!("{}/api/v1/client/launch/redeem", base);
    let agent = ureq::AgentBuilder::new().timeout_connect(Duration::from_secs(10)).timeout_read(Duration::from_secs(20)).user_agent("SolarPlayer/1.1").build();
    let r = match agent.post(&url).set("Content-Type", "application/json").send_string(&json!({ "token": token }).to_string()) {
        Ok(r) => r,
        Err(ureq::Error::Status(_, resp)) => {
            let v: Value = resp.into_string().ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(Value::Null);
            return Err(v.get("error").and_then(|e| e.as_str()).unwrap_or("The launch was refused").to_string());
        }
        Err(e) => return Err(format!("Cannot reach Solar ({})", e)),
    };
    let v: Value = serde_json::from_str(&r.into_string().map_err(|e| e.to_string())?).map_err(|_| "Unexpected answer from Solar".to_string())?;
    if v.get("ticket").and_then(|x| x.as_str()).is_none() {
        return Err("The launch has no ticket".into());
    }
    Ok(v)
}

fn main() {
    let o = parse();
    if o.token.is_empty() {
        fail("No launch token");
    }
    let mut boot = match redeem(&o.base, &o.token) {
        Ok(v) => v,
        Err(e) => fail(&e),
    };
    if let Some(m) = boot.as_object_mut() {
        m.remove("success");
        m.insert("channel".into(), json!(o.channel));
    }
    let name = boot.get("placeName").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let script = format!("window.__solarBoot = {};", boot);
    let event_loop = EventLoopBuilder::<String>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let title = if name.is_empty() { o.title.clone() } else { format!("{} - {}", name, o.title) };
    let window = match WindowBuilder::new().with_title(title).with_inner_size(LogicalSize::new(1024.0, 640.0)).build(&event_loop) {
        Ok(w) => w,
        Err(e) => fail(&format!("No window ({})", e)),
    };
    let url = format!("{}/desktop-host.html", o.base);
    let builder = WebViewBuilder::new().with_url(&url).with_initialization_script(&script).with_ipc_handler(move |req| {
        let _ = proxy.send_event(req.body().to_string());
    });
    #[cfg(target_os = "linux")]
    let webview = {
        use tao::platform::unix::WindowExtUnix;
        use wry::WebViewBuilderExtUnix;
        builder.build_gtk(window.default_vbox().unwrap())
    };
    #[cfg(not(target_os = "linux"))]
    let webview = builder.build(&window);
    let _webview = match webview {
        Ok(w) => w,
        Err(e) => fail(&format!("The web view could not start ({})", e)),
    };
    event_loop.run(move |event, _, flow| {
        *flow = ControlFlow::Wait;
        match event {
            Event::UserEvent(m) => {
                if m == "ready" {
                    say("SOLAR_READY");
                } else if let Some(e) = m.strip_prefix("error:") {
                    fail(e);
                }
            }
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => *flow = ControlFlow::Exit,
            _ => {}
        }
    });
}
