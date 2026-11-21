use mlua::{Function, Lua, Table, Value};
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use swirl_core::classes;
use swirl_core::instance::{DataModel, InstId, Val};

#[derive(Clone, Debug)]
pub struct LogLine {
    pub kind: String,
    pub msg: String,
    pub id: u32,
}

#[derive(Clone, Debug)]
pub enum Wire {
    Nil,
    B(bool),
    N(f64),
    S(String),
    I(u32),
    V3([f64; 3]),
    Cf([f64; 12]),
    C3([f64; 3]),
    V2([f64; 2]),
    Ud2([f64; 4]),
    E(String, f64),
    List(Vec<Wire>),
}

enum Q {
    Start(u32),
    Stop(u32),
    Event(u32, String, Vec<Wire>),
}

pub enum Req {
    LoadCharacter(u32),
    Debris(u32, f64),
}

struct State {
    reqs: Vec<Req>,
    queue: Vec<Q>,
    subs: HashSet<String>,
    logs: Vec<LogLine>,
    running: HashSet<u32>,
    t0: std::time::Instant,
}

pub struct Host {
    lua: Lua,
    dm: Rc<RefCell<DataModel>>,
    st: Rc<RefCell<State>>,
    step: Function,
}

const METHODS: [&str; 33] = [
    "GetChildren", "children", "getChildren", "FindFirstChild", "findFirstChild", "FindFirstChildOfClass", "FindFirstChildWhichIsA", "GetDescendants", "GetFullName", "IsA", "isA",
    "IsDescendantOf", "IsAncestorOf", "Clone", "clone", "Destroy", "destroy", "ClearAllChildren", "GetService", "service", "getService", "WaitForChild", "Remove", "remove",
    "TakeDamage", "LoadCharacter", "AddItem", "GetPlayers", "getPlayers", "GetPlayerFromCharacter", "getPlayerFromCharacter", "Fire", "Invoke",
];

fn event_names(class: &str) -> Vec<&'static str> {
    let mut v = vec!["Changed", "ChildAdded", "ChildRemoved", "DescendantAdded", "DescendantRemoving", "AncestryChanged"];
    if classes::is_a(class, "BasePart") {
        v.extend(["Touched", "TouchEnded"]);
    }
    if class == "Players" {
        v.extend(["PlayerAdded", "PlayerRemoving"]);
    }
    if class == "Player" {
        v.extend(["CharacterAdded", "CharacterRemoving", "Chatted"]);
    }
    if class == "Humanoid" {
        v.extend(["Died", "HealthChanged", "Running", "Jumping"]);
    }
    if class == "RunService" {
        v.extend(["Stepped", "Heartbeat", "RenderStepped"]);
    }
    if class == "BindableEvent" {
        v.push("Event");
    }
    if class == "RemoteEvent" {
        v.extend(["OnServerEvent", "OnClientEvent"]);
    }
    v
}


fn tagged(lua: &Lua, items: &[Value]) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    for (i, v) in items.iter().enumerate() {
        t.raw_set(i as i64, v.clone())?;
    }
    Ok(t)
}

fn s(lua: &Lua, v: &str) -> mlua::Result<Value> {
    Ok(Value::String(lua.create_string(v)?))
}

fn enc_into(lua: &Lua, out: &mut Vec<Value>, w: &Wire) -> mlua::Result<()> {
    match w {
        Wire::Nil => out.push(s(lua, "nil")?),
        Wire::B(b) => {
            out.push(s(lua, "b")?);
            out.push(Value::Boolean(*b));
        }
        Wire::N(n) => {
            out.push(s(lua, "n")?);
            out.push(Value::Number(*n));
        }
        Wire::S(x) => {
            out.push(s(lua, "s")?);
            out.push(s(lua, x)?);
        }
        Wire::I(i) => {
            out.push(s(lua, "i")?);
            out.push(Value::Number(*i as f64));
        }
        Wire::V3(v) => {
            out.push(s(lua, "v3")?);
            for x in v {
                out.push(Value::Number(*x));
            }
        }
        Wire::Cf(v) => {
            out.push(s(lua, "cf")?);
            for x in v {
                out.push(Value::Number(*x));
            }
        }
        Wire::C3(v) => {
            out.push(s(lua, "c3")?);
            for x in v {
                out.push(Value::Number(*x));
            }
        }
        Wire::V2(v) => {
            out.push(s(lua, "v2")?);
            for x in v {
                out.push(Value::Number(*x));
            }
        }
        Wire::Ud2(v) => {
            out.push(s(lua, "ud2")?);
            for x in v {
                out.push(Value::Number(*x));
            }
        }
        Wire::E(ty, n) => {
            out.push(s(lua, "e")?);
            out.push(s(lua, ty)?);
            out.push(Value::Number(*n));
        }
        Wire::List(l) => {
            out.push(s(lua, "l")?);
            out.push(Value::Number(l.len() as f64));
            for x in l {
                enc_into(lua, out, x)?;
            }
        }
    }
    Ok(())
}

fn decode_args(args: &[Value]) -> Vec<Wire> {
    let mut out = vec![];
    let mut i = 0;
    let num = |v: &Value| match v {
        Value::Number(n) => *n,
        Value::Integer(n) => *n as f64,
        _ => 0.0,
    };
    while i < args.len() {
        let tag = match &args[i] {
            Value::String(t) => t.to_str().map(|x| x.to_string()).unwrap_or_default(),
            _ => {
                i += 1;
                continue;
            }
        };
        i += 1;
        match tag.as_str() {
            "nil" => out.push(Wire::Nil),
            "b" => {
                out.push(Wire::B(matches!(args.get(i), Some(Value::Boolean(true)))));
                i += 1;
            }
            "n" => {
                out.push(Wire::N(args.get(i).map(num).unwrap_or(0.0)));
                i += 1;
            }
            "s" => {
                out.push(Wire::S(match args.get(i) {
                    Some(Value::String(x)) => x.to_str().map(|x| x.to_string()).unwrap_or_default(),
                    _ => String::new(),
                }));
                i += 1;
            }
            "i" => {
                out.push(Wire::I(args.get(i).map(num).unwrap_or(0.0) as u32));
                i += 1;
            }
            "v3" | "c3" => {
                let v = [args.get(i).map(num).unwrap_or(0.0), args.get(i + 1).map(num).unwrap_or(0.0), args.get(i + 2).map(num).unwrap_or(0.0)];
                out.push(if tag == "v3" { Wire::V3(v) } else { Wire::C3(v) });
                i += 3;
            }
            "cf" => {
                let mut v = [0.0; 12];
                for k in 0..12 {
                    v[k] = args.get(i + k).map(num).unwrap_or(0.0);
                }
                out.push(Wire::Cf(v));
                i += 12;
            }
            "v2" => {
                out.push(Wire::V2([args.get(i).map(num).unwrap_or(0.0), args.get(i + 1).map(num).unwrap_or(0.0)]));
                i += 2;
            }
            "ud2" => {
                out.push(Wire::Ud2([args.get(i).map(num).unwrap_or(0.0), args.get(i + 1).map(num).unwrap_or(0.0), args.get(i + 2).map(num).unwrap_or(0.0), args.get(i + 3).map(num).unwrap_or(0.0)]));
                i += 4;
            }
            "bc" | "e" => {
                out.push(Wire::N(args.get(i + if tag == "e" { 1 } else { 0 }).map(num).unwrap_or(0.0)));
                i += if tag == "e" { 2 } else { 1 };
            }
            _ => out.push(Wire::Nil),
        }
    }
    out
}

fn val_to_wire(ty: &str, enum_type: Option<&str>, v: &Val) -> Wire {
    match (ty, v) {
        ("enum", Val::Num(n)) if enum_type.is_some() => Wire::E(enum_type.unwrap().to_string(), *n),
        ("V2", Val::Arr(a)) if a.len() >= 2 => Wire::V2([a[0], a[1]]),
        ("UD2", Val::Arr(a)) if a.len() >= 4 => Wire::Ud2([a[0], a[1], a[2], a[3]]),
        ("bool", Val::Bool(b)) => Wire::B(*b),
        ("string", Val::Str(x)) | ("Content", Val::Str(x)) => Wire::S(x.clone()),
        ("int", Val::Num(n)) | ("number", Val::Num(n)) | ("BC", Val::Num(n)) | ("enum", Val::Num(n)) => Wire::N(*n),
        ("V3", Val::Arr(a)) if a.len() >= 3 => Wire::V3([a[0], a[1], a[2]]),
        ("C3", Val::Arr(a)) if a.len() >= 3 => Wire::C3([a[0], a[1], a[2]]),
        ("CF", Val::Arr(a)) if a.len() >= 12 => {
            let mut c = [0.0; 12];
            c.copy_from_slice(&a[..12]);
            Wire::Cf(c)
        }
        ("Ref", Val::Ref(Some(id))) => Wire::I(*id),
        _ => Wire::Nil,
    }
}

fn wire_to_val(ty: &str, w: &Wire) -> Option<Val> {
    Some(match (ty, w) {
        ("bool", Wire::B(b)) => Val::Bool(*b),
        ("string", Wire::S(x)) | ("Content", Wire::S(x)) => Val::Str(x.clone()),
        ("string", Wire::N(n)) => Val::Str(n.to_string()),
        ("int", Wire::N(n)) => Val::Num(n.trunc()),
        ("number", Wire::N(n)) | ("enum", Wire::N(n)) | ("BC", Wire::N(n)) => Val::Num(*n),
        ("V3", Wire::V3(a)) | ("C3", Wire::C3(a)) => Val::Arr(a.to_vec()),
        ("V2", Wire::V2(a)) => Val::Arr(a.to_vec()),
        ("UD2", Wire::Ud2(a)) => Val::Arr(a.to_vec()),
        ("CF", Wire::Cf(a)) => Val::Arr(a.to_vec()),
        ("Ref", Wire::I(i)) => Val::Ref(Some(*i)),
        ("Ref", Wire::Nil) => Val::Ref(None),
        _ => return None,
    })
}

fn push_event(st: &Rc<RefCell<State>>, id: u32, name: &str, args: Vec<Wire>) {
    let key = format!("{}:{}", id, name);
    if st.borrow().subs.contains(&key) {
        st.borrow_mut().queue.push(Q::Event(id, name.to_string(), args));
    }
}

fn after_parent_change(st: &Rc<RefCell<State>>, dm: &DataModel, id: InstId, old: Option<InstId>, new: Option<InstId>) {
    if let Some(o) = old {
        if Some(o) != new {
            push_event(st, o, "ChildRemoved", vec![Wire::I(id)]);
        }
    }
    if let Some(n) = new {
        if Some(n) != old {
            push_event(st, n, "ChildAdded", vec![Wire::I(id)]);
            let mut a = dm.get(n).and_then(|i| i.parent);
            let mut cur = Some(n);
            while let Some(c) = cur {
                push_event(st, c, "DescendantAdded", vec![Wire::I(id)]);
                cur = dm.get(c).and_then(|i| i.parent);
                if cur == a {
                    a = None;
                }
            }
        }
    }
}

impl Host {
    pub fn new(dm: Rc<RefCell<DataModel>>) -> mlua::Result<Host> {
        let lua = unsafe { Lua::unsafe_new() };
        let st = Rc::new(RefCell::new(State { reqs: vec![], queue: vec![], subs: HashSet::new(), logs: vec![], running: HashSet::new(), t0: std::time::Instant::now() }));
        let j = lua.create_table()?;
        {
            let dm = dm.clone();
            j.set(
                "get",
                lua.create_function(move |lua, (id, key): (u32, String)| {
                    let d = dm.borrow();
                    let inst = match d.get(id) {
                        Some(i) if !i.destroyed => i,
                        _ => return Ok(Value::Table(tagged(lua, &[s(lua, "err")?, s(lua, "Instance no longer exists")?])?)),
                    };
                    if key == "Parent" {
                        return Ok(Value::Table(match inst.parent {
                            Some(p) => tagged(lua, &[s(lua, "i")?, Value::Number(p as f64)])?,
                            None => tagged(lua, &[s(lua, "nil")?])?,
                        }));
                    }
                    if key == "__ownerPlayer" {
                        return Ok(Value::Table(tagged(lua, &[s(lua, "nil")?])?));
                    }
                    if key == "ClassName" || key == "className" {
                        return Ok(Value::Table(tagged(lua, &[s(lua, "s")?, s(lua, &inst.class)?])?));
                    }
                    if key == "Name" {
                        return s(lua, &inst.name);
                    }
                    if METHODS.contains(&key.as_str()) {
                        return Ok(Value::Table(tagged(lua, &[s(lua, "m")?, s(lua, &key)?])?));
                    }
                    let cap = {
                        let mut c = key.clone();
                        if let Some(f) = c.get(..1) {
                            c = f.to_uppercase() + &c[1..];
                        }
                        c
                    };
                    if event_names(&inst.class).contains(&cap.as_str()) {
                        return Ok(Value::Table(tagged(lua, &[s(lua, "ev")?, s(lua, &cap)?])?));
                    }
                    if let Some(pn) = classes::canonical_prop(&inst.class, &key) {
                        if let Some(p) = classes::prop_info(&inst.class, &pn) {
                            let mut out = vec![];
                            let w = inst.props.get(&pn).map(|v| val_to_wire(&p.ty, p.enum_type.as_deref(), v)).unwrap_or(Wire::Nil);
                            enc_into(lua, &mut out, &w)?;
                            return Ok(Value::Table(tagged(lua, &out)?));
                        }
                    }
                    if let Some(c) = d.find_child(id, &key, false) {
                        return Ok(Value::Table(tagged(lua, &[s(lua, "i")?, Value::Number(c as f64)])?));
                    }
                    if id == d.root {
                        let name = if key == "workspace" { "Workspace".to_string() } else { key.clone() };
                        if let Some(&sid) = d.services.get(&name) {
                            return Ok(Value::Table(tagged(lua, &[s(lua, "i")?, Value::Number(sid as f64)])?));
                        }
                    }
                    Ok(Value::Table(tagged(lua, &[s(lua, "err")?, s(lua, &format!("{} is not a valid member of {}", key, inst.class))?])?))
                })?,
            )?;
        }
        {
            let dm = dm.clone();
            let st2 = st.clone();
            j.set(
                "set",
                lua.create_function(move |lua, (id, key, rest): (u32, String, mlua::Variadic<Value>)| {
                    let args = decode_args(&rest);
                    let w = args.first().cloned().unwrap_or(Wire::Nil);
                    let mut d = dm.borrow_mut();
                    if d.get(id).map(|i| i.destroyed).unwrap_or(true) {
                        return Ok(Value::String(lua.create_string("Instance no longer exists")?));
                    }
                    if key == "Parent" {
                        let np = match w {
                            Wire::I(p) => Some(p),
                            Wire::Nil => None,
                            _ => return Ok(Value::String(lua.create_string("bad argument #3 to '?' (Object expected)")?)),
                        };
                        let old = d.get(id).and_then(|i| i.parent);
                        if let Err(e) = d.set_parent(id, np) {
                            return Ok(Value::String(lua.create_string(&e)?));
                        }
                        after_parent_change(&st2, &d, id, old, np);
                        return Ok(Value::Nil);
                    }
                    let class = d.get(id).unwrap().class.clone();
                    let pn = match classes::canonical_prop(&class, &key) {
                        Some(p) => p,
                        None => return Ok(Value::String(lua.create_string(&format!("{} is not a valid member of {}", key, class))?)),
                    };
                    let ty = classes::prop_info(&class, &pn).map(|p| p.ty.clone()).unwrap_or_default();
                    match wire_to_val(&ty, &w) {
                        Some(v) => {
                            let _ = d.set_prop(id, &pn, v);
                            push_event(&st2, id, "Changed", vec![Wire::S(pn)]);
                            Ok(Value::Nil)
                        }
                        None => Ok(Value::String(lua.create_string(&format!("invalid value for {}", key))?)),
                    }
                })?,
            )?;
        }
        {
            let dm = dm.clone();
            let st2 = st.clone();
            j.set(
                "call",
                lua.create_function(move |lua, (id, method, rest): (u32, String, mlua::Variadic<Value>)| {
                    let args = decode_args(&rest);
                    let mut d = dm.borrow_mut();
                    let err = |m: &str| -> mlua::Result<Value> { Ok(Value::Table(tagged(lua, &[s(lua, "err")?, s(lua, m)?])?)) };
                    if d.get(id).map(|i| i.destroyed).unwrap_or(true) {
                        return err("Instance no longer exists");
                    }
                    let ok = |w: Wire| -> mlua::Result<Value> {
                        let mut out = vec![s(lua, "ok")?];
                        enc_into(lua, &mut out, &w)?;
                        Ok(Value::Table(tagged(lua, &out)?))
                    };
                    let name_arg = |i: usize| match args.get(i) {
                        Some(Wire::S(x)) => x.clone(),
                        _ => String::new(),
                    };
                    match method.as_str() {
                        "__source" => {
                            let src = match d.get_prop(id, "Source") {
                                Some(Val::Str(x)) => x.clone(),
                                _ => String::new(),
                            };
                            ok(Wire::S(src))
                        }
                        "__coreCode" | "__robloxLocked" => Ok(Value::Boolean(false)),
                        "__TweenState" => ok(Wire::Nil),
                        "__DestroySelf" | "Destroy" | "destroy" | "Remove" | "remove" => {
                            let old = d.get(id).and_then(|i| i.parent);
                            d.destroy(id);
                            after_parent_change(&st2, &d, id, old, None);
                            ok(Wire::Nil)
                        }
                        "GetChildren" | "children" | "getChildren" => {
                            let kids: Vec<Wire> = d.get(id).unwrap().children.iter().map(|c| Wire::I(*c)).collect();
                            ok(Wire::List(kids))
                        }
                        "GetDescendants" => {
                            let mut v = vec![];
                            d.descendants(id, &mut v);
                            ok(Wire::List(v.into_iter().map(Wire::I).collect()))
                        }
                        "FindFirstChild" | "findFirstChild" => {
                            let rec = matches!(args.get(1), Some(Wire::B(true)));
                            match d.find_child(id, &name_arg(0), rec) {
                                Some(c) => ok(Wire::I(c)),
                                None => ok(Wire::Nil),
                            }
                        }
                        "FindFirstChildOfClass" => match d.find_child_of_class(id, &name_arg(0)) {
                            Some(c) => ok(Wire::I(c)),
                            None => ok(Wire::Nil),
                        },
                        "FindFirstChildWhichIsA" => {
                            let base = name_arg(0);
                            let found = d.get(id).unwrap().children.iter().copied().find(|c| d.is_a(*c, &base));
                            match found {
                                Some(c) => ok(Wire::I(c)),
                                None => ok(Wire::Nil),
                            }
                        }
                        "GetFullName" => ok(Wire::S(d.full_name(id))),
                        "IsA" | "isA" => {
                            let b = d.is_a(id, &name_arg(0));
                            Ok(Value::Boolean(b))
                        }
                        "IsDescendantOf" => {
                            let b = match args.first() {
                                Some(Wire::I(o)) => d.is_descendant_of(id, *o),
                                _ => false,
                            };
                            Ok(Value::Boolean(b))
                        }
                        "IsAncestorOf" => {
                            let b = match args.first() {
                                Some(Wire::I(o)) => d.is_descendant_of(*o, id),
                                _ => false,
                            };
                            Ok(Value::Boolean(b))
                        }
                        "Clone" | "clone" => match d.clone_tree(id) {
                            Some(c) => ok(Wire::I(c)),
                            None => ok(Wire::Nil),
                        },
                        "ClearAllChildren" => {
                            let kids = d.get(id).unwrap().children.clone();
                            for k in kids {
                                d.destroy(k);
                                after_parent_change(&st2, &d, k, Some(id), None);
                            }
                            ok(Wire::Nil)
                        }
                        "TakeDamage" => {
                            let n = match args.first() {
                                Some(Wire::N(n)) => *n,
                                _ => 0.0,
                            };
                            let cur = match d.get_prop(id, "Health") {
                                Some(Val::Num(h)) => *h,
                                _ => 100.0,
                            };
                            let nh = (cur - n).max(0.0);
                            let _ = d.set_prop(id, "Health", Val::Num(nh));
                            push_event(&st2, id, "HealthChanged", vec![Wire::N(nh)]);
                            ok(Wire::Nil)
                        }
                        "LoadCharacter" => {
                            st2.borrow_mut().reqs.push(Req::LoadCharacter(id));
                            ok(Wire::Nil)
                        }
                        "AddItem" => {
                            if let (Some(Wire::I(t)), Some(Wire::N(s))) = (args.first(), args.get(1)) {
                                st2.borrow_mut().reqs.push(Req::Debris(*t, *s));
                            }
                            ok(Wire::Nil)
                        }
                        "GetPlayers" | "getPlayers" => {
                            let l: Vec<Wire> = d.get(id).unwrap().children.iter().copied().filter(|c| d.get(*c).map(|i| i.class == "Player").unwrap_or(false)).map(Wire::I).collect();
                            ok(Wire::List(l))
                        }
                        "GetPlayerFromCharacter" | "getPlayerFromCharacter" => {
                            let target = match args.first() {
                                Some(Wire::I(m)) => *m,
                                _ => 0,
                            };
                            let found = d.get(id).unwrap().children.iter().copied().find(|c| matches!(d.get_prop(*c, "Character"), Some(Val::Ref(Some(m))) if *m == target));
                            match found {
                                Some(p) => ok(Wire::I(p)),
                                None => ok(Wire::Nil),
                            }
                        }
                        "GetService" | "service" | "getService" => {
                            let n = name_arg(0);
                            match d.service(&n) {
                                Ok(sid) => ok(Wire::I(sid)),
                                Err(_) => err(&format!("{} is not a valid Service name", n)),
                            }
                        }
                        _ => err(&format!("{} is not a valid member of {}", method, d.get(id).unwrap().class)),
                    }
                })?,
            )?;
        }
        {
            let dm = dm.clone();
            let st2 = st.clone();
            j.set(
                "new",
                lua.create_function(move |lua, (cls, parent): (String, Option<u32>)| {
                    let mut d = dm.borrow_mut();
                    match d.create(&cls, None) {
                        Ok(id) => {
                            if let Some(p) = parent {
                                if d.set_parent(id, Some(p)).is_ok() {
                                    after_parent_change(&st2, &d, id, None, Some(p));
                                }
                            }
                            Ok(Value::Number(id as f64))
                        }
                        Err(_) => Ok(Value::String(lua.create_string(&format!("Unable to create an Instance of type \"{}\"", cls))?)),
                    }
                })?,
            )?;
        }
        {
            let st2 = st.clone();
            j.set("sub", lua.create_function(move |_, (id, name): (u32, String)| {
                st2.borrow_mut().subs.insert(format!("{}:{}", id, name));
                Ok(())
            })?)?;
            let st2 = st.clone();
            j.set("unsub", lua.create_function(move |_, (id, name): (u32, String)| {
                st2.borrow_mut().subs.remove(&format!("{}:{}", id, name));
                Ok(())
            })?)?;
            let st2 = st.clone();
            j.set(
                "poll",
                lua.create_function(move |lua, ()| {
                    let q: Vec<Q> = std::mem::take(&mut st2.borrow_mut().queue);
                    let mut out: Vec<Value> = vec![];
                    for item in q {
                        match item {
                            Q::Start(id) => {
                                out.push(s(lua, "start")?);
                                out.push(Value::Number(id as f64));
                            }
                            Q::Stop(id) => {
                                out.push(s(lua, "stop")?);
                                out.push(Value::Number(id as f64));
                            }
                            Q::Event(id, name, args) => {
                                out.push(s(lua, "ev")?);
                                out.push(Value::Number(id as f64));
                                out.push(s(lua, &name)?);
                                out.push(Value::Number(args.len() as f64));
                                for a in &args {
                                    enc_into(lua, &mut out, a)?;
                                }
                            }
                        }
                    }
                    Ok(Value::Table(tagged(lua, &out)?))
                })?,
            )?;
            j.set("alive", lua.create_function(|_, _id: u32| Ok(true))?)?;
            let st2 = st.clone();
            j.set("running", lua.create_function(move |_, id: u32| Ok(st2.borrow().running.contains(&id)))?)?;
            let st2 = st.clone();
            j.set(
                "log",
                lua.create_function(move |_, (kind, msg, id): (String, String, Option<u32>)| {
                    st2.borrow_mut().logs.push(LogLine { kind, msg, id: id.unwrap_or(0) });
                    Ok(())
                })?,
            )?;
            let st2 = st.clone();
            j.set(
                "now",
                lua.create_function(move |_, what: Option<String>| {
                    if what.as_deref() == Some("tick") {
                        Ok(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0))
                    } else {
                        Ok(st2.borrow().t0.elapsed().as_secs_f64())
                    }
                })?,
            )?;
        }
        j.set("side", "server")?;
        j.set("data", include_str!("../data/luadata.lua"))?;
        let rt: Table = lua.load(include_str!("../data/runtime.lua")).set_name("=runtime").call(j)?;
        let step: Function = rt.get("step")?;
        Ok(Host { lua, dm, st, step })
    }

    pub fn scan_scripts(&mut self) {
        let d = self.dm.borrow();
        let mut all = vec![];
        d.descendants(d.root, &mut all);
        let mut st = self.st.borrow_mut();
        for id in all {
            if let Some(i) = d.get(id) {
                if i.class == "Script" && !matches!(i.props.get("Disabled"), Some(Val::Bool(true))) {
                    st.running.insert(id);
                    st.queue.push(Q::Start(id));
                }
            }
        }
    }

    pub fn tick(&mut self, t: f64) {
        if let Err(e) = self.step.call::<()>(t) {
            self.st.borrow_mut().logs.push(LogLine { kind: "error".into(), msg: format!("scheduler: {}", e), id: 0 });
        }
    }

    pub fn take_logs(&mut self) -> Vec<LogLine> {
        std::mem::take(&mut self.st.borrow_mut().logs)
    }

    pub fn fire(&mut self, id: u32, name: &str, args: Vec<Wire>) {
        push_event(&self.st, id, name, args);
    }

    pub fn subscribed(&self, name: &str) -> Vec<u32> {
        let suffix = format!(":{}", name);
        self.st.borrow().subs.iter().filter(|k| k.ends_with(&suffix)).filter_map(|k| k.split(':').next().and_then(|n| n.parse().ok())).collect()
    }

    pub fn take_requests(&mut self) -> Vec<Req> {
        std::mem::take(&mut self.st.borrow_mut().reqs)
    }

    pub fn lua(&self) -> &Lua {
        &self.lua
    }
}
