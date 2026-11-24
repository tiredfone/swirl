use crate::client_api::ClientApi;
use crate::host::{Res, Wire};
use swirl_core::classes::{enum_value, is_a};
use swirl_core::instance::{DataModel, InstId, Val};
use swirl_physics::cf;

fn n(args: &[Wire], i: usize) -> Option<f64> {
    match args.get(i) {
        Some(Wire::N(v)) => Some(*v),
        Some(Wire::E(_, v)) => Some(*v),
        _ => None,
    }
}

fn en(ty: &str, name: &str) -> Wire {
    Wire::E(ty.to_string(), enum_value(ty, name).unwrap_or(0) as f64)
}

fn camera_view(d: &DataModel, screen: (f64, f64)) -> Option<(InstId, [f64; 12], f64, f64, f64)> {
    let cam = d.services.get("Workspace").and_then(|w| match d.get_prop(*w, "CurrentCamera") {
        Some(Val::Ref(Some(c))) => Some(*c),
        _ => None,
    })?;
    let c = match d.get_prop(cam, "CFrame") {
        Some(Val::Arr(a)) => cf::arr(a)?,
        _ => return None,
    };
    let fov = match d.get_prop(cam, "FieldOfView") {
        Some(Val::Num(v)) if *v > 0.0 => *v,
        _ => 70.0,
    };
    Some((cam, c, fov, screen.0, screen.1))
}

fn list_ids(w: Option<&Wire>) -> Vec<InstId> {
    match w {
        Some(Wire::List(l)) => l.iter().filter_map(|x| if let Wire::I(i) = x { Some(*i) } else { None }).collect(),
        Some(Wire::Dict(l)) => l.iter().filter_map(|(_, x)| if let Wire::I(i) = x { Some(*i) } else { None }).collect(),
        _ => vec![],
    }
}

impl ClientApi {
    pub fn call2018(&mut self, d: &mut DataModel, id: InstId, class: &str, method: &str, args: &[Wire]) -> Option<Res> {
        let ok = |w: Vec<Wire>| Some(Res::Ok(w));
        let nil = || Some(Res::Ok(vec![]));
        let screen = self.core.borrow().screen;
        match method {
            _ if class == "PlayerScripts" && (method.starts_with("Register") || method.starts_with("Clear") || method.starts_with("GetRegistered")) && method.contains("MovementMode") => {
                let kind = method.trim_start_matches("GetRegistered").trim_start_matches("Register").trim_start_matches("Clear").trim_end_matches('s').trim_end_matches("MovementMode");
                let ty = format!("{}MovementMode", kind);
                let key = (id, ty.clone());
                if method.starts_with("Register") {
                    let v = match args.first() {
                        Some(Wire::E(_, v)) | Some(Wire::N(v)) => Some(*v),
                        Some(Wire::S(name)) => enum_value(&ty, name).map(|v| v as f64),
                        _ => None,
                    };
                    if let Some(v) = v {
                        let mut c = self.core.borrow_mut();
                        let l = c.modes.entry(key).or_default();
                        if !l.contains(&v) {
                            l.push(v);
                        }
                    }
                    nil()
                } else if method.starts_with("Clear") {
                    self.core.borrow_mut().modes.remove(&key);
                    nil()
                } else {
                    let mut l = self.core.borrow().modes.get(&key).cloned().unwrap_or_default();
                    if l.is_empty() && ty.ends_with("CameraMovementMode") {
                        l = swirl_core::classes::table().enums.get(&ty).map(|e| e.iter().take(3).map(|x| x.1 as f64).collect()).unwrap_or_default();
                    }
                    ok(vec![Wire::List(l.into_iter().map(|v| Wire::E(ty.clone(), v)).collect())])
                }
            }
            "SetDefaultLoadingGuiRemoved" if class == "ReplicatedFirst" => {
                let mut c = self.core.borrow_mut();
                if !c.loading_removed {
                    c.loading_removed = true;
                    c.fire_later.push((id, "RemoveDefaultLoadingGuiSignal"));
                }
                nil()
            }
            "IsDefaultLoadingGuiRemoved" if class == "ReplicatedFirst" => ok(vec![Wire::B(self.core.borrow().loading_removed)]),
            "IsFinishedReplicating" if class == "ReplicatedFirst" => ok(vec![Wire::B(true)]),
            "GetAssetThumbnailAsync" if class == "AssetService" => ok(vec![Wire::S(String::new()), Wire::B(true)]),
            "AddCoreScriptLocal" if class == "ScriptContext" => {
                if let Some(Wire::S(name)) = args.first() {
                    let parent = match args.get(1) {
                        Some(Wire::I(p)) => Some(*p),
                        _ => None,
                    };
                    self.core.borrow_mut().local_scripts.push((name.clone(), parent));
                }
                nil()
            }
            "GetFFlag" | "GetFastFlag" => ok(vec![Wire::B(matches!(args.first(), Some(Wire::S(s)) if s == "DebugDisableLogServiceExecuteScript"))]),
            "GetLastInputType" if class == "UserInputService" => {
                let v = self.core.borrow().last_input;
                ok(vec![if v == 0.0 { en("UserInputType", "Keyboard") } else { Wire::E("UserInputType".into(), v) }])
            }
            "GetPlatform" if class == "UserInputService" => ok(vec![en("Platform", "Windows")]),
            "GetMouseDelta" if class == "UserInputService" => {
                let mut c = self.core.borrow_mut();
                let v = c.mouse_delta;
                c.mouse_delta = [0.0, 0.0];
                ok(vec![Wire::V2(v)])
            }
            "IsMouseButtonPressed" if class == "UserInputService" => {
                let v = n(args, 0).unwrap_or(0.0);
                let b = if v == enum_value("UserInputType", "MouseButton1").unwrap_or(-1) as f64 { 0 } else if v == enum_value("UserInputType", "MouseButton3").unwrap_or(-1) as f64 { 1 } else if v == enum_value("UserInputType", "MouseButton2").unwrap_or(-1) as f64 { 2 } else { -1 };
                ok(vec![Wire::B(self.core.borrow().buttons_down.contains(&b))])
            }
            "GetKeysPressed" if class == "UserInputService" => ok(vec![Wire::List(vec![])]),
            "GetFocusedTextBox" if class == "UserInputService" => nil(),
            "GetStringForKeyCode" if class == "UserInputService" => {
                let v = n(args, 0).unwrap_or(0.0) as u32;
                let s = if (32..127).contains(&v) { (v as u8 as char).to_ascii_uppercase().to_string() } else { String::new() };
                ok(vec![Wire::S(s)])
            }
            "GetDeviceAcceleration" | "GetDeviceGravity" | "GetDeviceRotation" | "SetNavigationGamepad" if class == "UserInputService" => nil(),
            "GetUserCFrame" if class == "UserInputService" || class == "VRService" => ok(vec![Wire::Cf([0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0])]),
            "GetUserCFrameEnabled" if class == "VRService" => ok(vec![Wire::B(false)]),
            "RecenterUserHeadCFrame" | "RequestNavigation" | "SetTouchpadMode" if class == "VRService" => nil(),
            "GetTouchpadMode" if class == "VRService" => ok(vec![en("VRTouchpadMode", "Left")]),
            "IsVibrationSupported" | "IsMotorSupported" if class == "HapticService" => ok(vec![Wire::B(false)]),
            "SetMotor" if class == "HapticService" => nil(),
            "GetMotor" if class == "HapticService" => ok(vec![Wire::N(0.0)]),
            "GetPlatformId" if class == "PlatformService" => ok(vec![Wire::S(String::new())]),
            "PopupProfileUI" | "PopupGameInviteUI" if class == "PlatformService" => nil(),
            "BeginGetPMPCreatorId" if class == "PlatformService" => ok(vec![Wire::N(1.0)]),
            "BeginCheckXboxPrivilege" if class == "PlatformService" => ok(vec![Wire::Dict(vec![("PrivilegeCheckResult".into(), Wire::S("NoIssue".into()))])]),
            "BeginPlatformStorePurchase" if class == "PlatformService" => ok(vec![Wire::N(-1.0)]),
            "GetGuiInset" if class == "GuiService" => {
                let g = self.core.borrow().gui_inset;
                ok(vec![Wire::V2([g[0], g[1]]), Wire::V2([g[2], g[3]])])
            }
            "GetBrickCount" if class == "GuiService" => {
                let ws = *d.services.get("Workspace")?;
                let mut ds = vec![];
                d.descendants(ws, &mut ds);
                ok(vec![Wire::N(ds.iter().filter(|p| d.get(**p).map(|i| is_a(&i.class, "BasePart")).unwrap_or(false)).count() as f64)])
            }
            "GetErrorMessage" if class == "GuiService" => ok(vec![Wire::S(String::new())]),
            "ToggleFullscreen" | "ToggleFullScreen" if class == "GuiService" => nil(),
            "SetMenuIsOpen" if class == "GuiService" => {
                let open = matches!(args.first(), Some(Wire::B(true)));
                let _ = d.set_prop(id, "MenuIsOpen", Val::Bool(open));
                self.core.borrow_mut().fire_later.push((id, if open { "MenuOpened" } else { "MenuClosed" }));
                nil()
            }
            "GetPlayer" | "CloseConnection" if class == "NetworkReplicator" => nil(),
            "GetTopbarTransparency" if class == "PlayerGui" => ok(vec![Wire::N(0.5)]),
            "SetTopbarTransparency" if class == "PlayerGui" => nil(),
            "SetCoreGuiEnabled" | "SetCore" if class == "StarterGui" => nil(),
            "Set3dRenderingEnabled" if class == "RunService" => nil(),
            "GetTextSize" if class == "TextService" => {
                let text = match args.first() {
                    Some(Wire::S(s)) => s.clone(),
                    _ => String::new(),
                };
                let size = n(args, 1).unwrap_or(14.0);
                let font = n(args, 2).unwrap_or(0.0);
                let bound = match args.get(3) {
                    Some(Wire::V2(v)) => *v,
                    _ => [1e9, 1e9],
                };
                let m = self.measure.as_ref()?;
                let l = d.create_any("TextLabel", None).ok()?;
                let _ = d.set_prop(l, "Text", Val::Str(text));
                let _ = d.set_prop(l, "Font", Val::Num(font));
                let _ = d.set_prop(l, "TextSize", Val::Num(size));
                if bound[0] < 1e8 {
                    let _ = d.set_prop(l, "TextWrapped", Val::Bool(true));
                    let _ = d.set_prop(l, "Size", Val::Arr(vec![0.0, bound[0], 0.0, bound[1].min(1e6)]));
                }
                let b = crate::gui_layout::text_bounds(d, l, m.as_ref(), [screen.0, screen.1]);
                d.destroy_quiet(l);
                ok(vec![Wire::V2(b)])
            }
            "FilterStringAsync" if class == "TextService" => ok(vec![args.first().cloned().unwrap_or(Wire::S(String::new()))]),
            "GetUserThumbnailAsync" if class == "Players" => {
                let uid = match args.first() {
                    Some(Wire::N(n)) => *n as i64,
                    Some(Wire::I(n)) => *n as i64,
                    _ => 0,
                };
                let head = match args.get(1) {
                    Some(Wire::E(_, v)) | Some(Wire::N(v)) => *v == 0.0,
                    _ => true,
                };
                let c = self.core.borrow();
                let t = if head { &c.thumb_urls[0] } else { &c.thumb_urls[1] };
                let t = if t.is_empty() { &c.thumb_urls[0] } else { t };
                ok(vec![Wire::S(t.replace("{id}", &uid.max(0).to_string())), Wire::B(true)])
            }
            "GetCharacterAppearanceInfoAsync" | "GetFriendsAsync" if class == "Players" => ok(vec![Wire::Dict(vec![])]),
            "SetCharacterAppearanceTimeout" if class == "Players" => nil(),
            "GetGameSessionID" if class == "Player" => ok(vec![Wire::S(String::new())]),
            "IsUserAvailableForExperiment" if class == "Player" => ok(vec![Wire::B(false)]),
            "GetStateEnabled" if class == "Humanoid" => ok(vec![Wire::B(true)]),
            "GetAccessories" if class == "Humanoid" => {
                let par = d.get(id)?.parent?;
                let kids = d.get(par)?.children.clone();
                ok(vec![Wire::List(kids.into_iter().filter(|c| d.get(*c).map(|i| is_a(&i.class, "Accoutrement")).unwrap_or(false)).map(Wire::I).collect())])
            }
            "AddAccessory" if class == "Humanoid" => {
                if let (Some(Wire::I(a)), Some(par)) = (args.first(), d.get(id).and_then(|i| i.parent)) {
                    let _ = d.set_parent(*a, Some(par));
                }
                nil()
            }
            "RemoveAccessories" if class == "Humanoid" => {
                if let Some(par) = d.get(id).and_then(|i| i.parent) {
                    let kids = d.get(par).map(|i| i.children.clone()).unwrap_or_default();
                    for c in kids {
                        if d.get(c).map(|i| is_a(&i.class, "Accoutrement")).unwrap_or(false) {
                            d.destroy(c);
                        }
                    }
                }
                nil()
            }
            "GetLimb" if class == "Humanoid" => {
                let name = match args.first() {
                    Some(Wire::I(p)) => d.name(*p).to_string(),
                    _ => String::new(),
                };
                let v = match name.as_str() {
                    "Head" => "Head",
                    "Torso" => "Torso",
                    "Left Arm" => "LeftArm",
                    "Right Arm" => "RightArm",
                    "Left Leg" => "LeftLeg",
                    "Right Leg" => "RightLeg",
                    _ => "Unknown",
                };
                ok(vec![en("Limb", v)])
            }
            "GetBodyPartR15" if class == "Humanoid" => ok(vec![en("BodyPartR15", "Unknown")]),
            "BuildRigFromAttachments" if class == "Humanoid" => nil(),
            "GetMoveVelocity" if class == "Humanoid" => {
                let v = match d.get_prop(id, "MoveDirection") {
                    Some(Val::Arr(a)) if a.len() >= 3 => [a[0], a[1], a[2]],
                    _ => [0.0; 3],
                };
                ok(vec![Wire::V3(v)])
            }
            "ScreenPointToRay" | "ViewportPointToRay" if class == "Camera" => {
                let (x, y) = (n(args, 0).unwrap_or(0.0), n(args, 1).unwrap_or(0.0));
                let depth = n(args, 2).unwrap_or(0.0);
                let (_, c, fov, w, h) = camera_view(d, screen)?;
                let t = (fov * std::f64::consts::PI / 360.0).tan();
                let aspect = w / h.max(1.0);
                let v = [(x / w * 2.0 - 1.0) * aspect * t, -(y / h) * 2.0 * t + t, -1.0];
                let dv = cf::vector(&c, v[0], v[1], v[2]);
                let l = (dv[0] * dv[0] + dv[1] * dv[1] + dv[2] * dv[2]).sqrt().max(1e-12);
                let dir = [dv[0] / l, dv[1] / l, dv[2] / l];
                let o = [c[0] + dir[0] * depth, c[1] + dir[1] * depth, c[2] + dir[2] * depth];
                ok(vec![Wire::List(vec![Wire::V3(o), Wire::V3(dir)])])
            }
            "WorldToScreenPoint" | "WorldToViewportPoint" if class == "Camera" => {
                let p = match args.first() {
                    Some(Wire::V3(p)) => *p,
                    _ => return ok(vec![Wire::V3([0.0; 3]), Wire::B(false)]),
                };
                let (_, c, fov, w, h) = camera_view(d, screen)?;
                let lc = cf::point(&cf::inv(&c), p[0], p[1], p[2]);
                let depth = -lc[2];
                let t = (fov * std::f64::consts::PI / 360.0).tan();
                let aspect = w / h.max(1.0);
                let dd = if depth.abs() < 1e-9 { 1e-9 } else { depth };
                let x = (lc[0] / (dd * aspect * t) * 0.5 + 0.5) * w;
                let y = (-lc[1] / (dd * t) * 0.5 + 0.5) * h;
                let inside = depth > 0.0 && x >= 0.0 && x <= w && y >= 0.0 && y <= h;
                ok(vec![Wire::V3([x, y, depth]), Wire::B(inside)])
            }
            "GetRenderCFrame" if class == "Camera" || is_a(class, "BasePart") => match d.get_prop(id, "CFrame") {
                Some(Val::Arr(a)) => cf::arr(a).and_then(|c| ok(vec![Wire::Cf(c)])),
                _ => nil(),
            },
            "GetLargestCutoffDistance" if class == "Camera" => {
                let ph = self.core.borrow().ph.clone()?;
                let c = match d.get_prop(id, "CFrame") {
                    Some(Val::Arr(a)) => cf::arr(a)?,
                    _ => return ok(vec![Wire::N(0.0)]),
                };
                let fc = match d.get_prop(id, "Focus") {
                    Some(Val::Arr(a)) if a.len() >= 3 => [a[0], a[1], a[2]],
                    _ => return ok(vec![Wire::N(0.0)]),
                };
                let dir = [c[0] - fc[0], c[1] - fc[1], c[2] - fc[2]];
                let len = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
                if len < 0.01 {
                    return ok(vec![Wire::N(0.0)]);
                }
                let mut ignore = list_ids(args.first());
                for _ in 0..8 {
                    let hit = ph.borrow().raycast(d, fc, dir, &ignore, None);
                    let part = match hit.part {
                        Some(p) => p,
                        None => return ok(vec![Wire::N(0.0)]),
                    };
                    if matches!(d.get_prop(part, "Transparency"), Some(Val::Num(t)) if *t >= 0.95) {
                        ignore.push(part);
                        continue;
                    }
                    let dist = ((hit.point[0] - fc[0]).powi(2) + (hit.point[1] - fc[1]).powi(2) + (hit.point[2] - fc[2]).powi(2)).sqrt();
                    return ok(vec![Wire::N((len - dist + 0.2).max(0.0))]);
                }
                ok(vec![Wire::N(0.0)])
            }
            "GetRoll" | "GetPanSpeed" | "GetTiltSpeed" if class == "Camera" => ok(vec![Wire::N(0.0)]),
            "SetRoll" | "PanUnits" | "TiltUnits" | "Zoom" if class == "Camera" => nil(),
            "GetPartsObscuringTarget" if class == "Camera" => ok(vec![Wire::List(vec![])]),
            "FindPartOnRayWithWhitelist" if class == "Workspace" => {
                let ph = self.core.borrow().ph.clone()?;
                let (o, dv) = match args.first() {
                    Some(Wire::List(l)) => match (l.first(), l.get(1)) {
                        (Some(Wire::V3(o)), Some(Wire::V3(dv))) => (*o, *dv),
                        _ => return ok(vec![Wire::Nil, Wire::V3([0.0; 3]), Wire::V3([0.0, 1.0, 0.0])]),
                    },
                    _ => return ok(vec![Wire::Nil, Wire::V3([0.0; 3]), Wire::V3([0.0, 1.0, 0.0])]),
                };
                let mut allow = std::collections::HashSet::new();
                for r in list_ids(args.get(1)) {
                    allow.insert(r);
                    let mut ds = vec![];
                    d.descendants(r, &mut ds);
                    allow.extend(ds);
                }
                let ws = *d.services.get("Workspace")?;
                let mut all = vec![];
                d.descendants(ws, &mut all);
                let ignore: Vec<InstId> = all.into_iter().filter(|p| d.get(*p).map(|i| is_a(&i.class, "BasePart")).unwrap_or(false) && !allow.contains(p)).collect();
                let hit = ph.borrow().raycast(d, o, dv, &ignore, None);
                ok(vec![hit.part.map(Wire::I).unwrap_or(Wire::Nil), Wire::V3(hit.point), Wire::V3(hit.normal)])
            }
            "FindPartsInRegion3WithIgnoreList" | "FindPartsInRegion3WithWhiteList" if class == "Workspace" => ok(vec![Wire::List(vec![])]),
            "IsRegion3Empty" | "IsRegion3EmptyWithIgnoreList" if class == "Workspace" => ok(vec![Wire::B(true)]),
            "ArePartsTouchingOthers" if class == "Workspace" => ok(vec![Wire::B(false)]),
            "PGSIsEnabled" | "ExperimentalSolverIsEnabled" if class == "Workspace" => ok(vec![Wire::B(true)]),
            "JoinToOutsiders" | "UnjoinFromOutsiders" if class == "Workspace" => nil(),
            "CanSetNetworkOwnership" if is_a(class, "BasePart") => ok(vec![Wire::B(false), Wire::S("Cannot change network ownership".into())]),
            "SetNetworkOwner" | "SetNetworkOwnershipAuto" | "GetNetworkOwner" if is_a(class, "BasePart") => nil(),
            "GetNetworkOwnershipAuto" if is_a(class, "BasePart") => ok(vec![Wire::B(true)]),
            "IsGrounded" if is_a(class, "BasePart") => ok(vec![Wire::B(matches!(d.get_prop(id, "Anchored"), Some(Val::Bool(true))))]),
            "GetCorescriptLocalizations" if class == "LocalizationService" => ok(vec![Wire::List(vec![])]),
            "GetTranslatorForPlayer" if class == "LocalizationService" => nil(),
            "FilterStringForBroadcast" if class == "Chat" => ok(vec![args.get(0).cloned().unwrap_or(Wire::S(String::new()))]),
            "CanUserChatAsync" | "CanUsersChatAsync" if class == "Chat" => ok(vec![Wire::B(true)]),
            "PlayLocalSound" if class == "SoundService" => nil(),
            "GetListener" if class == "SoundService" => ok(vec![en("ListenerType", "Camera"), Wire::Nil]),
            "SetListener" if class == "SoundService" => nil(),
            "PreloadAsync" if class == "ContentProvider" => nil(),
            "GetLocalPlayerTeleportData" if class == "TeleportService" => nil(),
            "GetTotalMemoryUsageMb" if class == "Stats" => ok(vec![Wire::N(120.0)]),
            "GetMemoryUsageMbForTag" if class == "Stats" => ok(vec![Wire::N(0.0)]),
            "GetValue" if class == "StatsItem" => ok(vec![Wire::N(0.0)]),
            "GetValueString" if class == "StatsItem" => ok(vec![Wire::S("0".into())]),
            "ReportCounter" | "TrackEvent" | "SetRBXEvent" | "SetRBXEventStream" | "UpdateHeartbeatObject" | "ReportInfluxSeries" | "ReleaseRBXEventStream" | "ReportStats" if class == "AnalyticsService" => nil(),
            "GetSessionId" if class == "AnalyticsService" => ok(vec![Wire::S(String::new())]),
            "ReportGoogleAnalyticsEvent" | "ReportInGoogleAnalytics" => nil(),
            "GetNotificationTypeList" if class == "GuiService" => ok(vec![Wire::Dict(vec![])]),
            "GetEmotesMenuOpen" if class == "GuiService" => ok(vec![Wire::B(false)]),
            "GetSafeZoneOffsets" if class == "GuiService" => ok(vec![Wire::Dict(["top", "bottom", "left", "right"].iter().map(|k| (k.to_string(), Wire::N(0.0))).collect())]),
            "GetCameraYInvertValue" | "GetOnboardingCompleted" if class == "UserGameSettings" => ok(vec![Wire::N(0.0)]),
            "SetCameraYInvertValue" | "SetCameraYInvertVisible" | "SetGamepadCameraSensitivityVisible" | "SetOnboardingCompleted" if class == "UserGameSettings" => nil(),
            "IsUserFeatureEnabled" if class == "UserSettings" => ok(vec![Wire::B(false)]),
            "SetPlayerIsUsing" | "SetIsExemptFromUGCAnalytics" => nil(),
            "GetCurrentPlayers" if class == "Dialog" => ok(vec![Wire::List(vec![])]),
            _ => None,
        }
    }
}
