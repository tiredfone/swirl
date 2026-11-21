pub mod host;

pub use host::{Host, LogLine};

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;
    use swirl_core::classes;
    use swirl_core::instance::{DataModel, Val};

    fn game() -> Rc<RefCell<DataModel>> {
        let mut dm = DataModel::new("server");
        for s in ["Workspace", "Players", "Lighting", "ServerScriptService"] {
            dm.service(s).unwrap();
        }
        Rc::new(RefCell::new(dm))
    }

    #[test]
    fn lua51_semantics() {
        let lua = mlua::Lua::new();
        let v: String = lua.load("return _VERSION").eval().unwrap();
        assert_eq!(v, "Lua 5.1");
        assert!(classes::class_info("Part").is_some());
    }

    #[test]
    fn runs_a_script() {
        let dm = game();
        let ssv = dm.borrow().services["ServerScriptService"];
        let sc = dm.borrow_mut().create("Script", Some(ssv)).unwrap();
        let src = r#"
            print("hello from lua")
            local p = Instance.new("Part")
            p.Name = "Made"
            p.Size = Vector3.new(4, 2, 6)
            p.Anchored = true
            p.Parent = workspace
            wait(0.05)
            local q = Instance.new("Part", workspace)
            q.Name = "Second"
            print(#workspace:GetChildren(), workspace:FindFirstChild("Made").Name)
        "#;
        dm.borrow_mut().set_prop(sc, "Source", Val::Str(src.into())).unwrap();
        let mut host = Host::new(dm.clone()).unwrap();
        host.scan_scripts();
        let mut t = 0.0;
        for _ in 0..30 {
            t += 0.02;
            host.tick(t);
        }
        let logs = host.take_logs();
        let text: Vec<String> = logs.iter().map(|l| l.msg.clone()).collect();
        assert!(text.iter().any(|m| m == "hello from lua"), "{:?}", text);
        assert!(text.iter().any(|m| m.starts_with("2") || m.starts_with("3")), "{:?}", text);
        let d = dm.borrow();
        let ws = d.services["Workspace"];
        assert!(d.find_child(ws, "Made", false).is_some());
        assert!(d.find_child(ws, "Second", false).is_some());
        let made = d.find_child(ws, "Made", false).unwrap();
        assert_eq!(d.get_prop(made, "Size"), Some(&Val::Arr(vec![4.0, 2.0, 6.0])));
    }
}
