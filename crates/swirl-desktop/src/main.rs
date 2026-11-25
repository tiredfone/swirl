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

fn post_json(base: &str, path: &str, body: Value) -> Result<Value, String> {
    let url = format!("{}{}", base, path);
    let agent = ureq::AgentBuilder::new().timeout_connect(Duration::from_secs(10)).timeout_read(Duration::from_secs(20)).user_agent("SolarPlayer/1.1").build();
    let r = match agent.post(&url).set("Content-Type", "application/json").send_string(&body.to_string()) {
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

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(if b[i] == b'+' { b' ' } else { b[i] });
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn play_target(base: &str, to: &str) -> Option<(i64, String)> {
    let rest = to.strip_prefix(base)?.strip_prefix("/play/")?;
    let (path, query) = match rest.split_once('?') {
        Some((p, q)) => (p, q),
        None => (rest, ""),
    };
    let path = path.split('#').next().unwrap_or("").trim_end_matches('/');
    let id: i64 = path.parse().ok()?;
    let mut server = String::new();
    for pair in query.split('#').next().unwrap_or("").split('&') {
        if let Some(v) = pair.strip_prefix("server=") {
            server = percent_decode(v);
        }
    }
    Some((id, server))
}

fn finish_boot(mut boot: Value, channel: &str) -> Value {
    if let Some(m) = boot.as_object_mut() {
        m.remove("success");
        m.remove("refresh");
        m.insert("channel".into(), json!(channel));
    }
    boot
}

fn main() {
    let o = parse();
    if o.token.is_empty() {
        fail("No launch token");
    }
    let first = match post_json(&o.base, "/api/v1/client/launch/redeem", json!({ "token": o.token })) {
        Ok(v) => v,
        Err(e) => fail(&e),
    };
    if first.get("ticket").and_then(|x| x.as_str()).is_none() {
        fail("The launch has no ticket");
    }
    let refresh = first.get("refresh").and_then(|x| x.as_str()).unwrap_or("").to_string();
    let mut place = first.get("placeId").and_then(|x| x.as_i64()).unwrap_or(0);
    let mut server = first.get("server").and_then(|x| x.as_str()).unwrap_or("").to_string();
    let mut first_boot = Some(finish_boot(first, &o.channel));
    let event_loop = EventLoopBuilder::<String>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let window = match WindowBuilder::new().with_title(o.title.clone()).with_inner_size(LogicalSize::new(1024.0, 640.0)).build(&event_loop) {
        Ok(w) => w,
        Err(e) => fail(&format!("No window ({})", e)),
    };
    let url = format!("{}/desktop-host.html", o.base);
    let nav_proxy = proxy.clone();
    let host = url.clone();
    let base = o.base.clone();
    let builder = WebViewBuilder::new().with_url(&url).with_ipc_handler(move |req| {
        let _ = proxy.send_event(req.body().to_string());
    }).with_navigation_handler(move |to: String| {
        if to.starts_with(&host) || to.starts_with("about:") || to.starts_with("data:") || to.starts_with("blob:") {
            return true;
        }
        if let Some((id, server)) = play_target(&base, &to) {
            let _ = nav_proxy.send_event(format!("teleport:{}:{}", id, server));
        } else {
            let _ = nav_proxy.send_event("leave".to_string());
        }
        false
    });
    #[cfg(target_os = "linux")]
    let webview = {
        use tao::platform::unix::WindowExtUnix;
        use wry::WebViewBuilderExtUnix;
        builder.build_gtk(window.default_vbox().unwrap())
    };
    #[cfg(not(target_os = "linux"))]
    let webview = builder.build(&window);
    let webview = match webview {
        Ok(w) => w,
        Err(e) => fail(&format!("The web view could not start ({})", e)),
    };
    let loop_proxy = event_loop.create_proxy();
    let mut pending = false;
    let base = o.base.clone();
    let channel_name = o.channel.clone();
    let title_base = o.title.clone();
    event_loop.run(move |event, _, flow| {
        *flow = ControlFlow::Wait;
        match event {
            Event::UserEvent(m) => {
                if m == "leave" {
                    *flow = ControlFlow::Exit;
                } else if let Some(rest) = m.strip_prefix("teleport:") {
                    let mut it = rest.splitn(2, ':');
                    place = it.next().and_then(|v| v.parse().ok()).unwrap_or(place);
                    server = it.next().unwrap_or("").to_string();
                    first_boot = None;
                    pending = false;
                    let _ = webview.load_url(&format!("{}/desktop-host.html", base));
                } else if m == "boot" {
                    if let Some(b) = first_boot.take() {
                        deliver(&webview, &window, &title_base, &b);
                    } else if !pending {
                        pending = true;
                        let (b, r, p, s, c, px) = (base.clone(), refresh.clone(), place, server.clone(), channel_name.clone(), loop_proxy.clone());
                        std::thread::spawn(move || {
                            let mut body = json!({ "refresh": r, "place": p });
                            if !s.is_empty() {
                                body["server"] = json!(s);
                            }
                            let msg = match post_json(&b, "/api/v1/client/relaunch", body) {
                                Ok(v) => format!("bootdata:{}", finish_boot(v, &c)),
                                Err(e) => format!("error:{}", e),
                            };
                            let _ = px.send_event(msg);
                        });
                    }
                } else if let Some(j) = m.strip_prefix("bootdata:") {
                    pending = false;
                    if let Ok(b) = serde_json::from_str::<Value>(j) {
                        deliver(&webview, &window, &title_base, &b);
                    }
                } else if m == "ready" {
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

fn deliver(webview: &wry::WebView, window: &tao::window::Window, title: &str, boot: &Value) {
    let name = boot.get("placeName").and_then(|v| v.as_str()).unwrap_or("");
    window.set_title(&if name.is_empty() { title.to_string() } else { format!("{} - {}", name, title) });
    let _ = webview.evaluate_script(&format!("window.__solarBoot = {}; window.dispatchEvent(new Event('solarboot'));", boot));
}
