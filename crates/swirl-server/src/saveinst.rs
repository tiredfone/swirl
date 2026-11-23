use std::collections::HashMap;
use swirl_core::classes;
use swirl_core::instance::{DataModel, InstId, Val};

const HEADER: &str = "<roblox xmlns:xmime=\"http://www.w3.org/2005/05/xmlmime\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xsi:noNamespaceSchemaLocation=\"http://www.roblox.com/roblox.xsd\" version=\"4\">\n\t<External>null</External>\n\t<External>nil</External>\n";
const SKIP: [&str; 11] = ["Archivable", "RobloxLocked", "DistributedGameTime", "LocalPlayer", "NumPlayers", "JobId", "PlaceId", "CreatorId", "GameId", "Parent", "ClassName"];

fn esc(s: &str) -> String {
    let mut o = String::new();
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            c if (c as u32) < 0x20 && !matches!(c, '\t' | '\n' | '\r') => {}
            c => o.push(c),
        }
    }
    o
}

fn f(v: f64) -> String {
    if v == f64::INFINITY {
        return "INF".into();
    }
    if v == f64::NEG_INFINITY {
        return "-INF".into();
    }
    if !v.is_finite() {
        return "NAN".into();
    }
    let s = format!("{:.8e}", v);
    let r: f64 = s.parse().unwrap_or(v);
    let t = format!("{}", r);
    t
}

fn color32(c: &[f64]) -> String {
    let b = |x: f64| (x * 255.0).round().clamp(0.0, 255.0) as u32;
    let g = |i: usize| c.get(i).copied().unwrap_or(0.0);
    (((255u32) << 24) | (b(g(0)) << 16) | (b(g(1)) << 8) | b(g(2))).to_string()
}

fn serial_name(d: &DataModel, id: InstId, class: &str, name: &str) -> String {
    if d.is_a(id, "BasePart") {
        match name {
            "Size" => return "size".into(),
            "Color" => return "Color3uint8".into(),
            _ => {}
        }
    }
    if class == "Humanoid" && name == "Health" {
        return "Health_XML".into();
    }
    name.to_string()
}

fn value(ty: &str, name: &str, v: &Val, refs: &HashMap<InstId, String>) -> Option<String> {
    let n = esc(name);
    let arr = |v: &Val| -> Vec<f64> {
        match v {
            Val::Arr(a) => a.clone(),
            _ => vec![],
        }
    };
    let g = |a: &Vec<f64>, i: usize| a.get(i).copied().unwrap_or(0.0);
    let num = match v {
        Val::Num(n) => *n,
        Val::Bool(b) => *b as i32 as f64,
        _ => 0.0,
    };
    Some(match ty {
        "bool" => format!("<bool name=\"{}\">{}</bool>", n, matches!(v, Val::Bool(true))),
        "int" => format!("<int name=\"{}\">{}</int>", n, num.trunc() as i64),
        "number" => format!("<float name=\"{}\">{}</float>", n, f(num)),
        "string" => format!("<string name=\"{}\">{}</string>", n, esc(match v { Val::Str(s) => s, _ => "" })),
        "Content" => match v {
            Val::Str(s) if !s.is_empty() => format!("<Content name=\"{}\"><url>{}</url></Content>", n, esc(s)),
            _ => format!("<Content name=\"{}\"><null></null></Content>", n),
        },
        "enum" => format!("<token name=\"{}\">{}</token>", n, num.trunc() as i64),
        "BC" => format!("<int name=\"{}\">{}</int>", n, num.trunc() as i64),
        "C3" => format!("<Color3 name=\"{}\">{}</Color3>", n, color32(&arr(v))),
        "V3" => {
            let a = arr(v);
            format!("<Vector3 name=\"{}\"><X>{}</X><Y>{}</Y><Z>{}</Z></Vector3>", n, f(g(&a, 0)), f(g(&a, 1)), f(g(&a, 2)))
        }
        "V2" => {
            let a = arr(v);
            format!("<Vector2 name=\"{}\"><X>{}</X><Y>{}</Y></Vector2>", n, f(g(&a, 0)), f(g(&a, 1)))
        }
        "CF" => {
            let a = arr(v);
            let k = ["X", "Y", "Z", "R00", "R01", "R02", "R10", "R11", "R12", "R20", "R21", "R22"];
            format!("<CoordinateFrame name=\"{}\">{}</CoordinateFrame>", n, k.iter().enumerate().map(|(i, x)| format!("<{0}>{1}</{0}>", x, f(g(&a, i)))).collect::<String>())
        }
        "UD2" => {
            let a = arr(v);
            format!("<UDim2 name=\"{}\"><XS>{}</XS><XO>{}</XO><YS>{}</YS><YO>{}</YO></UDim2>", n, f(g(&a, 0)), g(&a, 1).trunc() as i64, f(g(&a, 2)), g(&a, 3).trunc() as i64)
        }
        "UD" => {
            let a = arr(v);
            format!("<UDim name=\"{}\"><S>{}</S><O>{}</O></UDim>", n, f(g(&a, 0)), g(&a, 1).trunc() as i64)
        }
        "Ref" => {
            let r = match v {
                Val::Ref(Some(i)) => refs.get(i).cloned().unwrap_or_else(|| "null".into()),
                _ => "null".into(),
            };
            format!("<Ref name=\"{}\">{}</Ref>", n, r)
        }
        _ => return None,
    })
}

fn mark(d: &DataModel, id: InstId, saved: &mut Vec<InstId>) {
    let inst = match d.get(id) {
        Some(i) if !i.destroyed => i,
        _ => return,
    };
    if matches!(inst.props.get("Archivable"), Some(Val::Bool(false))) {
        return;
    }
    saved.push(id);
    for c in &inst.children {
        mark(d, *c, saved);
    }
}

fn write(d: &DataModel, id: InstId, depth: usize, refs: &HashMap<InstId, String>, saved: &[InstId], out: &mut String) {
    let inst = match d.get(id) {
        Some(i) => i,
        None => return,
    };
    let pad = "\t".repeat(depth);
    out.push_str(&format!("{}<Item class=\"{}\" referent=\"{}\">\n{}\t<Properties>\n", pad, esc(&inst.class), refs[&id], pad));
    out.push_str(&format!("{}\t\t<string name=\"Name\">{}</string>\n", pad, esc(&inst.name)));
    if let Some(ci) = classes::class_info(&inst.class) {
        let mut names: Vec<&String> = ci.all.keys().collect();
        names.sort();
        for name in names {
            if SKIP.contains(&name.as_str()) || name == "Name" {
                continue;
            }
            let p = &ci.all[name];
            let v = match inst.props.get(name) {
                Some(v) => v.clone(),
                None => Val::from_default(&p.ty, &p.def),
            };
            if let Some(x) = value(&p.ty, &serial_name(d, id, &inst.class, name), &v, refs) {
                out.push_str(&format!("{}\t\t{}\n", pad, x));
            }
        }
    }
    out.push_str(&format!("{}\t</Properties>\n", pad));
    for c in &inst.children {
        if saved.contains(c) {
            write(d, *c, depth + 1, refs, saved, out);
        }
    }
    out.push_str(&format!("{}</Item>\n", pad));
}

pub fn write_model(d: &DataModel, top: InstId) -> String {
    let mut saved = vec![];
    mark(d, top, &mut saved);
    let refs: HashMap<InstId, String> = saved.iter().enumerate().map(|(i, id)| (*id, format!("RBX{:X}", i))).collect();
    let mut out = String::from(HEADER);
    if !saved.is_empty() {
        write(d, top, 1, &refs, &saved, &mut out);
    }
    out.push_str("</roblox>");
    out
}

pub fn load_model(d: &mut DataModel, xml: &str) -> Option<InstId> {
    let tree = swirl_core::rbx::parse(xml.as_bytes()).ok()?;
    let holder = d.create_any("Model", None).ok()?;
    let made = d.load_tree(&tree, holder);
    let top = made.first().copied();
    let kids = d.get(holder).map(|i| i.children.clone()).unwrap_or_default();
    let top = top.filter(|t| kids.contains(t)).or_else(|| kids.first().copied());
    if let Some(t) = top {
        let _ = d.set_parent(t, None);
    }
    for k in kids {
        if Some(k) != top {
            d.destroy(k);
        }
    }
    d.destroy(holder);
    top
}
