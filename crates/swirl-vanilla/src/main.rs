use std::collections::HashMap;
use swirl_render::assets::DirAssets;
use swirl_render::look::*;
use swirl_render::scene::*;

fn args() -> HashMap<String, String> {
    let mut m = HashMap::new();
    for a in std::env::args().skip(1) {
        if let Some(r) = a.strip_prefix("--") {
            match r.split_once('=') {
                Some((k, v)) => m.insert(k.to_string(), v.to_string()),
                None => m.insert(r.to_string(), "1".to_string()),
            };
        }
    }
    m
}

fn read_png(p: &str) -> (usize, usize, Vec<u8>) {
    let f = std::fs::File::open(p).unwrap_or_else(|e| panic!("{}: {}", p, e));
    let mut rd = png::Decoder::new(f).read_info().unwrap();
    let mut buf = vec![0; rd.output_buffer_size()];
    let i = rd.next_frame(&mut buf).unwrap();
    (i.width as usize, i.height as usize, buf)
}

fn diff(a: &str, b: &str, out: Option<&str>) {
    let (w, h, pa) = read_png(a);
    let (w2, h2, pb) = read_png(b);
    if w != w2 || h != h2 {
        println!("size mismatch {}x{} vs {}x{}", w, h, w2, h2);
        std::process::exit(2);
    }
    let mut sum = [0u64; 4];
    let mut over = 0usize;
    let mut maxd = 0i32;
    let mut amis = 0usize;
    let mut viz = vec![0u8; w * h * 4];
    for i in 0..w * h {
        let mut worst = 0i32;
        for c in 0..4 {
            let d = (pa[i * 4 + c] as i32 - pb[i * 4 + c] as i32).abs();
            sum[c] += d as u64;
            worst = worst.max(d);
        }
        if worst > 8 {
            over += 1;
        }
        maxd = maxd.max(worst);
        if (pa[i * 4 + 3] == 0) != (pb[i * 4 + 3] == 0) {
            amis += 1;
        }
        let v = (worst * 4).min(255) as u8;
        viz[i * 4] = v;
        viz[i * 4 + 1] = if worst > 8 { 0 } else { v };
        viz[i * 4 + 2] = if worst > 8 { 0 } else { v };
        viz[i * 4 + 3] = 255;
    }
    let n = (w * h) as f64;
    println!(
        "size={}x{} mae_rgba=[{:.4} {:.4} {:.4} {:.4}] pct_over8={:.4}% max={} alpha_mismatch={}",
        w,
        h,
        sum[0] as f64 / n,
        sum[1] as f64 / n,
        sum[2] as f64 / n,
        sum[3] as f64 / n,
        over as f64 * 100.0 / n,
        maxd,
        amis
    );
    if let Some(o) = out {
        std::fs::write(o, swirl_render::img::encode_png(w, h, &viz)).unwrap();
    }
}

fn main() {
    let raw: Vec<String> = std::env::args().collect();
    if raw.get(1).map(|s| s.as_str()) == Some("diff") {
        diff(&raw[2], &raw[3], raw.get(4).map(|s| s.as_str()));
        return;
    }
    let a = args();
    let data = a.get("data").cloned().unwrap_or("/var/www/html/core/vanilla-rcc/data".into());
    let root = a.get("assets").cloned().unwrap_or("/var/www/assets".into());
    let out = a.get("out").expect("--out required");
    let size = a.get("size").cloned().unwrap_or("352x352".into());
    let (w, h) = size.split_once('x').map(|(w, h)| (w.parse().unwrap(), h.parse().unwrap())).expect("bad size");
    let view = a.get("view").cloned().unwrap_or("body".into());
    let src = DirAssets::solar(&root);
    let ctx = Ctx::new(&src, &data);
    let base = Base::load(&data).unwrap_or_else(|e| panic!("{}", e));
    let mut look = Look::empty();
    if let Some(c) = a.get("colors") {
        let v: Vec<i64> = c.split(',').map(|s| s.trim().parse().unwrap()).collect();
        assert!(v.len() == 6, "six colours");
        let names = [HEAD, TORSO, LARM, RARM, LLEG, RLEG];
        for (j, k) in names.iter().enumerate() {
            look.colors[*k] = parse_color_int(v[j]);
        }
    }
    let mut ids: Vec<String> = vec![];
    if let Some(i) = a.get("ids") {
        for s in i.split(',').filter(|s| !s.is_empty()) {
            if !ids.contains(&s.to_string()) {
                ids.push(s.to_string());
            }
        }
    }
    let cfs = swirl_render::items::build_look(&ctx, &base, &mut look, &ids, &view);
    for n in &look.notes {
        eprintln!("note: {}", n);
    }
    let png = draw(&base, &look, &cfs, w, h, 4, &view);
    std::fs::write(out, png).unwrap();
}
