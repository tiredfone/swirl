use crate::api::{ApiState, SocialKind, SocialReq};
use serde_json::json;
use swirl_core::instance::{DataModel, InstId, Val};
use swirl_lua::host::{Res, Wire};
use swirl_physics::cf;

pub const METHODS: &[&str] = &[
    "IsLoaded", "GetMessage", "SetMessage", "ClearMessage", "Shutdown", "FinishShutdown", "FindService", "findService",
    "Chat", "chat", "TeamChat", "teamChat", "WhisperChat", "ReportAbuse", "GetUserThumbnailAsync", "SetChatStyle", "SetCharacterAppearanceTimeout",
    "IsFriendsWith", "isFriendsWith", "GetFriendsOnline", "GetFriendStatus", "RequestFriendship", "RevokeFriendship", "GetMouse", "Kick", "DistanceFromCharacter", "ClearCharacterAppearance", "JumpCharacter", "MoveCharacter", "GetGameSessionID", "IsUserAvailableForExperiment",
    "GetState", "ChangeState", "GetLimb", "GetBodyPartR15", "GetAccessories", "AddAccessory", "RemoveAccessories", "GetMoveVelocity", "BuildRigFromAttachments",
    "FindPartsInRegion3", "findPartsInRegion3", "FindPartsInRegion3WithIgnoreList", "FindPartsInRegion3WithWhiteList", "IsRegion3Empty", "IsRegion3EmptyWithIgnoreList", "ArePartsTouchingOthers",
    "Resize", "resize", "CanSetNetworkOwnership", "IsGrounded", "GetSunDirection", "GetMoonDirection", "GetTextSize",
    "CreateCollisionGroup", "CollisionGroupSetCollidable", "CollisionGroupsAreCollidable", "GetCollisionGroupId", "GetCollisionGroupName", "GetCollisionGroups",
    "GetMaxCollisionGroups", "SetPartCollisionGroup", "CollisionGroupContainsPart", "RemoveCollisionGroup", "RenameCollisionGroup",
    "__invokeClient", "RegisterLibrary", "AddCoreScript", "AddStarterScript", "GenerateGUID", "GetDisplayName", "SetDesiredAngle", "Disable", "ToggleSelect",
];

fn text(a: &[Wire], i: usize) -> String {
    match a.get(i) {
        Some(Wire::S(s)) => s.clone(),
        Some(Wire::N(n)) => n.to_string(),
        _ => String::new(),
    }
}

fn num(a: &[Wire], i: usize) -> f64 {
    match a.get(i) {
        Some(Wire::N(n)) if n.is_finite() => *n,
        Some(Wire::E(_, v)) => *v,
        _ => 0.0,
    }
}

fn ok(v: Vec<Wire>) -> Option<Res> {
    Some(Res::Ok(v))
}

fn err(m: &str) -> Option<Res> {
    Some(Res::Err(m.to_string()))
}

fn minutes(d: &DataModel, id: InstId) -> f64 {
    let tod = match d.get_prop(id, "TimeOfDay") {
        Some(Val::Str(s)) => s.clone(),
        _ => "14:00:00".into(),
    };
    let mut it = tod.trim().split(':').map(|x| x.trim().parse::<f64>().unwrap_or(0.0));
    let h = it.next().unwrap_or(0.0);
    let m = it.next().unwrap_or(0.0);
    let s = it.next().unwrap_or(0.0);
    h * 60.0 + m + s / 60.0
}

fn sun(d: &DataModel, id: InstId) -> [f64; 3] {
    let min = minutes(d, id);
    let lat = match d.get_prop(id, "GeographicLatitude") {
        Some(Val::Num(n)) => *n,
        _ => 41.7333,
    } * std::f64::consts::PI
        / 180.0;
    let h = ((min / 1440.0) * 2.0 - 1.0) * std::f64::consts::PI;
    let x = -h.sin();
    let y = h.cos() * (lat - 0.4).cos();
    let z = h.cos() * (lat - 0.4).sin();
    let l = (x * x + y * y + z * z).sqrt();
    let l = if l == 0.0 { 1.0 } else { l };
    [x / l, y / l, z / l]
}

struct Group {
    name: String,
    id: i32,
    mask: i32,
}

fn groups_of(d: &DataModel) -> Vec<Group> {
    let s = d.services.get("Workspace").and_then(|w| match d.get_prop(*w, "CollisionGroups") {
        Some(Val::Str(s)) => Some(s.clone()),
        _ => None,
    });
    let t = swirl_physics::groups::parse(&s.unwrap_or_default());
    t.groups.into_iter().map(|g| Group { name: g.name, id: g.id, mask: g.mask }).collect()
}

fn save_groups(d: &mut DataModel, mut g: Vec<Group>) {
    g.sort_by_key(|x| x.id);
    let s = g.iter().map(|x| format!("{}^{}^{}", x.name, x.id, x.mask)).collect::<Vec<_>>().join("\\");
    if let Some(w) = d.services.get("Workspace").copied() {
        let _ = d.set_prop(w, "CollisionGroups", Val::Str(s));
    }
}

fn need(g: &[Group], a: &[Wire], i: usize) -> Result<usize, String> {
    let n = text(a, i);
    g.iter().position(|x| x.name == n).ok_or_else(|| format!("Parameter {} must be a valid collision group name", i + 1))
}

fn clean(s: &str, max: usize) -> String {
    s.chars().filter(|c| !c.is_control()).take(max).collect::<String>().trim().to_string()
}

pub fn call(st: &mut ApiState, d: &mut DataModel, id: InstId, method: &str, args: &[Wire]) -> Option<Res> {
    let class = d.get(id).map(|i| i.class.clone()).unwrap_or_default();
    match method {
        "IsLoaded" if id == d.root => ok(vec![Wire::B(true)]),
        "GetMessage" if id == d.root => ok(vec![Wire::S(String::new())]),
        "SetMessage" if id == d.root => {
            st.broadcasts.push(json!({"t": "msg", "m": clean(&text(args, 0), 500)}));
            ok(vec![])
        }
        "ClearMessage" if id == d.root => {
            st.broadcasts.push(json!({"t": "msg", "m": null}));
            ok(vec![])
        }
        "Shutdown" | "FinishShutdown" if id == d.root => ok(vec![]),
        "FindService" | "findService" if id == d.root => {
            let n = match args.first() {
                Some(Wire::I(i)) => d.get(*i).map(|x| x.class.clone()).unwrap_or_default(),
                _ => text(args, 0),
            };
            ok(vec![d.services.get(&n).copied().map(Wire::I).unwrap_or(Wire::Nil)])
        }
        "Chat" | "chat" | "TeamChat" | "teamChat" | "WhisperChat" | "ReportAbuse" | "SetChatStyle" | "SetCharacterAppearanceTimeout" if class == "Players" => ok(vec![]),
        "GetUserThumbnailAsync" if class == "Players" => {
            let uid = num(args, 0) as i64;
            let t = if args.get(1).is_some() { num(args, 1) } else { 0.0 };
            let kind = if t == 0.0 { "headshot" } else { "avatar" };
            ok(vec![Wire::S(format!("/image/{}?userId={}", kind, uid)), Wire::B(true)])
        }
        "IsFriendsWith" | "isFriendsWith" if class == "Player" => {
            let me = match d.get_prop(id, "userId") {
                Some(Val::Num(n)) => *n as i64,
                _ => 0,
            };
            let other = num(args, 0) as i64;
            if me <= 0 || other <= 0 || me == other {
                return ok(vec![Wire::B(false)]);
            }
            st.token += 1;
            let token = st.token;
            st.social.push(SocialReq { token, body: json!({"action": "statuses", "userId": me, "others": [other]}), kind: SocialKind::Friend(other) });
            Some(Res::Async(token))
        }
        "GetFriendsOnline" if class == "Player" => {
            let me = match d.get_prop(id, "userId") {
                Some(Val::Num(n)) => *n as i64,
                _ => 0,
            };
            let max = if args.first().is_some() { num(args, 0) as i64 } else { 200 };
            st.token += 1;
            let token = st.token;
            st.social.push(SocialReq { token, body: json!({"action": "friendsOnline", "userId": me, "max": max}), kind: SocialKind::FriendsOnline });
            Some(Res::Async(token))
        }
        "GetFriendStatus" if class == "Player" => ok(vec![Wire::E("FriendStatus".into(), 1.0)]),
        "RequestFriendship" | "RevokeFriendship" if class == "Player" => {
            let from = match d.get_prop(id, "userId") {
                Some(Val::Num(n)) => *n as i64,
                _ => 0,
            };
            if let Some(Wire::I(o)) = args.first() {
                let to = match d.get_prop(*o, "userId") {
                    Some(Val::Num(n)) => *n as i64,
                    _ => 0,
                };
                if from > 0 && to > 0 && from != to {
                    st.token += 1;
                    let token = st.token;
                    let action = if method == "RequestFriendship" { "request" } else { "revoke" };
                    st.social.push(SocialReq { token, body: json!({"action": action, "from": from, "to": to}), kind: SocialKind::Fire });
                }
            }
            ok(vec![])
        }
        "GetMouse" if class == "Player" => {
            if let Some(m) = st.mice.get(&id).copied() {
                if d.get(m).map(|i| !i.destroyed).unwrap_or(false) {
                    return ok(vec![Wire::I(m)]);
                }
            }
            let m = d.create_any("PlayerMouse", None).ok()?;
            d.get_mut_name(m, "Mouse");
            d.set_prop_quiet(m, "Name", Val::Str("Mouse".into()));
            d.set_prop_quiet(m, "ViewSizeX", Val::Num(1024.0));
            d.set_prop_quiet(m, "ViewSizeY", Val::Num(768.0));
            st.mice.insert(id, m);
            ok(vec![Wire::I(m)])
        }
        "Kick" if class == "Player" => {
            let m = text(args, 0);
            st.kicks.push((id, if m.is_empty() { "You have been kicked from the game".to_string() } else { m }));
            ok(vec![])
        }
        "DistanceFromCharacter" if class == "Player" => {
            let p = match args.first() {
                Some(Wire::V3(p)) => *p,
                _ => return ok(vec![Wire::N(0.0)]),
            };
            let ch = match d.get_prop(id, "Character") {
                Some(Val::Ref(Some(c))) => *c,
                _ => return ok(vec![Wire::N(0.0)]),
            };
            let h = d.find_child(ch, "Head", false).or_else(|| d.find_child(ch, "Torso", false));
            let pos = match h.and_then(|h| d.get_prop(h, "CFrame")) {
                Some(Val::Arr(a)) if a.len() >= 3 => [a[0], a[1], a[2]],
                _ => return ok(vec![Wire::N(0.0)]),
            };
            ok(vec![Wire::N(((pos[0] - p[0]).powi(2) + (pos[1] - p[1]).powi(2) + (pos[2] - p[2]).powi(2)).sqrt())])
        }
        "ClearCharacterAppearance" if class == "Player" => {
            if let Some(Val::Ref(Some(c))) = d.get_prop(id, "Character").cloned() {
                let kids = d.get(c).map(|i| i.children.clone()).unwrap_or_default();
                for k in kids {
                    if d.is_a(k, "Accoutrement") || d.is_a(k, "Clothing") || matches!(d.get(k).map(|i| i.class.as_str()), Some("ShirtGraphic" | "BodyColors" | "CharacterMesh")) {
                        d.destroy(k);
                    }
                }
            }
            ok(vec![])
        }
        "JumpCharacter" | "MoveCharacter" if class == "Player" => ok(vec![]),
        "GetGameSessionID" if class == "Player" => ok(vec![Wire::S(String::new())]),
        "IsUserAvailableForExperiment" if class == "Player" => ok(vec![Wire::B(false)]),
        "GetState" if class == "Humanoid" => {
            let s = st.hum_state.get(&id).copied().unwrap_or(255);
            let v = if s == 255 { 8 } else { [8, 8, 3, 5, 13, 12, 15, 8, 4].get(s as usize).copied().unwrap_or(8) };
            ok(vec![Wire::E("HumanoidStateType".into(), v as f64)])
        }
        "ChangeState" | "BuildRigFromAttachments" if class == "Humanoid" => ok(vec![]),
        "GetLimb" if class == "Humanoid" => {
            let n = match args.first() {
                Some(Wire::I(p)) => d.name(*p).to_string(),
                _ => String::new(),
            };
            let v = match n.as_str() {
                "Head" => 0,
                "Torso" => 1,
                "Left Arm" => 2,
                "Right Arm" => 3,
                "Left Leg" => 4,
                "Right Leg" => 5,
                _ => 6,
            };
            ok(vec![Wire::E("Limb".into(), v as f64)])
        }
        "GetBodyPartR15" if class == "Humanoid" => ok(vec![Wire::E("BodyPartR15".into(), 15.0)]),
        "GetAccessories" if class == "Humanoid" => {
            let kids = d.get(id).and_then(|i| i.parent).and_then(|p| d.get(p)).map(|p| p.children.clone()).unwrap_or_default();
            ok(vec![Wire::List(kids.into_iter().filter(|k| d.is_a(*k, "Accoutrement")).map(Wire::I).collect())])
        }
        "AddAccessory" if class == "Humanoid" => {
            if let (Some(Wire::I(a)), Some(p)) = (args.first(), d.get(id).and_then(|i| i.parent)) {
                let _ = d.set_parent(*a, Some(p));
            }
            ok(vec![])
        }
        "RemoveAccessories" if class == "Humanoid" => {
            if let Some(p) = d.get(id).and_then(|i| i.parent) {
                let kids = d.get(p).map(|i| i.children.clone()).unwrap_or_default();
                for k in kids {
                    if d.is_a(k, "Accoutrement") {
                        d.destroy(k);
                    }
                }
            }
            ok(vec![])
        }
        "GetMoveVelocity" if class == "Humanoid" => {
            let v = match d.get_prop(id, "MoveDirection") {
                Some(Val::Arr(a)) if a.len() >= 3 => [a[0], a[1], a[2]],
                _ => [0.0; 3],
            };
            ok(vec![Wire::V3(v)])
        }
        "FindPartsInRegion3" | "findPartsInRegion3" | "FindPartsInRegion3WithIgnoreList" | "FindPartsInRegion3WithWhiteList" if class == "Workspace" => ok(vec![Wire::List(vec![])]),
        "IsRegion3Empty" | "IsRegion3EmptyWithIgnoreList" if class == "Workspace" => ok(vec![Wire::B(true)]),
        "ArePartsTouchingOthers" if class == "Workspace" => ok(vec![Wire::B(false)]),
        "Resize" | "resize" if d.is_a(id, "BasePart") => {
            let face = num(args, 0) as i64;
            let delta = num(args, 1);
            let mut s = match d.get_prop(id, "Size") {
                Some(Val::Arr(a)) if a.len() >= 3 => [a[0], a[1], a[2]],
                _ => [1.0; 3],
            };
            let axis = [0usize, 1, 2, 0, 1, 2][(face.rem_euclid(6)) as usize];
            let sign = if face < 3 { 1.0 } else { -1.0 };
            s[axis] = (s[axis] + delta).max(0.2);
            let c = match d.get_prop(id, "CFrame") {
                Some(Val::Arr(a)) if a.len() == 12 => {
                    let mut o = [0.0; 12];
                    o.copy_from_slice(a);
                    o
                }
                _ => cf::IDENTITY,
            };
            let mut off = [0.0; 3];
            off[axis] = sign * delta / 2.0;
            let w = cf::vector(&c, off[0], off[1], off[2]);
            let mut nc = c;
            nc[0] += w[0];
            nc[1] += w[1];
            nc[2] += w[2];
            let _ = d.set_prop(id, "Size", Val::Arr(s.to_vec()));
            let _ = d.set_prop(id, "CFrame", Val::Arr(nc.to_vec()));
            ok(vec![Wire::B(true)])
        }
        "CanSetNetworkOwnership" if d.is_a(id, "BasePart") => ok(vec![Wire::B(false), Wire::S("Cannot change network ownership".into())]),
        "IsGrounded" if d.is_a(id, "BasePart") => ok(vec![Wire::B(matches!(d.get_prop(id, "Anchored"), Some(Val::Bool(true))))]),
        "GetSunDirection" if class == "Lighting" => ok(vec![Wire::V3(sun(d, id))]),
        "GetMoonDirection" if class == "Lighting" => {
            let s = sun(d, id);
            ok(vec![Wire::V3([-s[0], -s[1], -s[2]])])
        }
        "GetTextSize" if class == "TextService" => {
            let t = text(args, 0);
            let px = { let n = num(args, 1); if n == 0.0 { 14.0 } else { n } };
            let lines: Vec<&str> = t.split('\n').collect();
            let w = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0) as f64 * px * 0.5;
            ok(vec![Wire::V2([w, lines.len() as f64 * px])])
        }
        "CreateCollisionGroup" if class == "PhysicsService" => {
            let name = text(args, 0);
            if name.is_empty() {
                return err("Collision group name must be a non-empty string");
            }
            let mut g = groups_of(d);
            if g.iter().any(|x| x.name == name) {
                return err("Could not create collision group, one with that name already exists.");
            }
            let mut gid = 1;
            while g.iter().any(|x| x.id == gid) {
                gid += 1;
            }
            if gid >= 32 {
                return err("Could not create collision group, the maximum number of collision groups has been reached.");
            }
            let mut mask: i32 = 1i32.wrapping_shl(gid as u32);
            for x in g.iter_mut() {
                x.mask |= 1i32.wrapping_shl(gid as u32);
                mask |= 1i32.wrapping_shl(x.id as u32);
            }
            g.push(Group { name, id: gid, mask });
            save_groups(d, g);
            ok(vec![Wire::N(gid as f64)])
        }
        "CollisionGroupSetCollidable" if class == "PhysicsService" => {
            let mut g = groups_of(d);
            let a = match need(&g, args, 0) {
                Ok(v) => v,
                Err(e) => return err(&e),
            };
            let b = match need(&g, args, 1) {
                Ok(v) => v,
                Err(e) => return err(&e),
            };
            let on = !matches!(args.get(2), Some(Wire::B(false)) | Some(Wire::Nil) | None);
            let (ia, ib) = (g[a].id, g[b].id);
            if on {
                g[a].mask |= 1i32.wrapping_shl(ib as u32);
                g[b].mask |= 1i32.wrapping_shl(ia as u32);
            } else {
                g[a].mask &= !1i32.wrapping_shl(ib as u32);
                g[b].mask &= !1i32.wrapping_shl(ia as u32);
            }
            save_groups(d, g);
            ok(vec![])
        }
        "CollisionGroupsAreCollidable" if class == "PhysicsService" => {
            let g = groups_of(d);
            let a = match need(&g, args, 0) {
                Ok(v) => v,
                Err(e) => return err(&e),
            };
            let b = match need(&g, args, 1) {
                Ok(v) => v,
                Err(e) => return err(&e),
            };
            let (ia, ib) = (g[a].id, g[b].id);
            let c = (ia == ib && ia == 0) || (((g[a].mask as u32) >> ib) & 1 == 1 && ((g[b].mask as u32) >> ia) & 1 == 1);
            ok(vec![Wire::B(c)])
        }
        "GetCollisionGroupId" if class == "PhysicsService" => {
            let g = groups_of(d);
            match need(&g, args, 0) {
                Ok(i) => ok(vec![Wire::N(g[i].id as f64)]),
                Err(e) => err(&e),
            }
        }
        "GetCollisionGroupName" if class == "PhysicsService" => {
            let g = groups_of(d);
            let want = num(args, 0) as i32;
            match g.iter().find(|x| x.id == want) {
                Some(x) => ok(vec![Wire::S(x.name.clone())]),
                None => err("Parameter 1 must be a valid collision group id"),
            }
        }
        "GetCollisionGroups" if class == "PhysicsService" => {
            let g = groups_of(d);
            ok(vec![Wire::List(
                g.iter().map(|x| Wire::Dict(vec![("name".into(), Wire::S(x.name.clone())), ("id".into(), Wire::N(x.id as f64)), ("mask".into(), Wire::N(x.mask as f64))])).collect(),
            )])
        }
        "GetMaxCollisionGroups" if class == "PhysicsService" => ok(vec![Wire::N(32.0)]),
        "SetPartCollisionGroup" if class == "PhysicsService" => {
            let p = match args.first() {
                Some(Wire::I(p)) if d.is_a(*p, "BasePart") => *p,
                _ => return err("Parameter 1 must be a BasePart"),
            };
            let g = groups_of(d);
            match need(&g, args, 1) {
                Ok(i) => {
                    let _ = d.set_prop(p, "CollisionGroupId", Val::Num(g[i].id as f64));
                    ok(vec![])
                }
                Err(e) => err(&e),
            }
        }
        "CollisionGroupContainsPart" if class == "PhysicsService" => {
            let p = match args.get(1) {
                Some(Wire::I(p)) if d.is_a(*p, "BasePart") => *p,
                _ => return err("Parameter 2 must be a BasePart"),
            };
            let g = groups_of(d);
            match need(&g, args, 0) {
                Ok(i) => {
                    let cur = match d.get_prop(p, "CollisionGroupId") {
                        Some(Val::Num(n)) => *n as i32,
                        _ => 0,
                    };
                    ok(vec![Wire::B(cur == g[i].id)])
                }
                Err(e) => err(&e),
            }
        }
        "RemoveCollisionGroup" if class == "PhysicsService" => {
            let g = groups_of(d);
            let i = match need(&g, args, 0) {
                Ok(v) => v,
                Err(e) => return err(&e),
            };
            let gid = g[i].id;
            if gid == 0 {
                return err("Cannot remove the default collision group");
            }
            let mut rest: Vec<Group> = g.into_iter().filter(|x| x.id != gid).collect();
            for x in rest.iter_mut() {
                x.mask &= !1i32.wrapping_shl(gid as u32);
            }
            save_groups(d, rest);
            let mut all = vec![];
            d.descendants(d.root, &mut all);
            for p in all {
                if d.is_a(p, "BasePart") && matches!(d.get_prop(p, "CollisionGroupId"), Some(Val::Num(n)) if *n as i32 == gid) {
                    let _ = d.set_prop(p, "CollisionGroupId", Val::Num(0.0));
                }
            }
            ok(vec![])
        }
        "RenameCollisionGroup" if class == "PhysicsService" => {
            let mut g = groups_of(d);
            let i = match need(&g, args, 0) {
                Ok(v) => v,
                Err(e) => return err(&e),
            };
            let to = text(args, 1);
            if to.is_empty() {
                return err("Collision group name must be a non-empty string");
            }
            if g.iter().any(|x| x.name == to && x.id != g[i].id) {
                return err("Could not rename collision group, one with that name already exists.");
            }
            g[i].name = to;
            save_groups(d, g);
            ok(vec![])
        }
        "__invokeClient" if class == "RemoteFunction" => {
            let pl = match args.first() {
                Some(Wire::I(p)) if d.get(*p).map(|i| i.class == "Player" && !i.destroyed && i.parent.is_some()).unwrap_or(false) => *p,
                _ => return ok(vec![Wire::B(false), Wire::S("Player is not in the game".into())]),
            };
            st.token += 1;
            let k = st.token;
            let rest = swirl_lua::wire_to_json(&args[1..]);
            st.directed.push((pl, json!({"t": "rfc", "id": id, "k": k, "a": rest})));
            let at = st.now_ms + 30000.0;
            st.invoke_out.insert(k, (pl, at));
            Some(Res::Async(k))
        }
        "RegisterLibrary" | "AddCoreScript" | "AddStarterScript" if class == "ScriptContext" => ok(vec![]),
        "GenerateGUID" if class == "HttpService" => {
            let wrap = !matches!(args.first(), Some(Wire::B(false)));
            let mut b = [0u8; 16];
            let mut seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|t| t.as_nanos() as u64).unwrap_or(1) ^ (st.token as u64).wrapping_mul(0x9E3779B97F4A7C15);
            for x in b.iter_mut() {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                *x = (seed >> 24) as u8;
            }
            b[6] = (b[6] & 0x0f) | 0x40;
            b[8] = (b[8] & 0x3f) | 0x80;
            let h: Vec<String> = b.iter().map(|x| format!("{:02X}", x)).collect();
            let g = format!("{}-{}-{}-{}-{}", h[0..4].concat(), h[4..6].concat(), h[6..8].concat(), h[8..10].concat(), h[10..16].concat());
            st.token += 1;
            ok(vec![Wire::S(if wrap { format!("{{{}}}", g) } else { g })])
        }
        "GetDisplayName" => ok(vec![Wire::S(d.name(id).to_string())]),
        "SetDesiredAngle" if class == "Motor" || class == "Motor6D" => {
            let _ = d.set_prop(id, "DesiredAngle", Val::Num(num(args, 0)));
            ok(vec![])
        }
        "Disable" | "ToggleSelect" if class == "HopperBin" => ok(vec![]),
        _ => None,
    }
}
