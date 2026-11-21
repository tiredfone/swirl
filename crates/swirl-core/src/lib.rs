pub mod brickcolors;
pub mod classes;
pub mod instance;
pub mod math;
pub mod rbx;

#[cfg(test)]
mod tests {
    use crate::classes::*;
    use crate::instance::*;

    #[test]
    fn class_table() {
        assert!(is_a("Part", "BasePart"));
        assert!(is_a("Part", "Instance"));
        assert!(is_creatable("Part"));
        assert!(!is_creatable("Workspace"));
        assert!(is_service("Workspace"));
        assert_eq!(canonical_prop("Part", "size").as_deref(), Some("Size"));
        assert_eq!(canonical_prop("Part", "nonsense"), None);
        assert_eq!(enum_name("Material", 256), Some("Plastic"));
        assert!(table().classes.len() > 300);
    }

    #[test]
    fn tree() {
        let mut dm = DataModel::new("server");
        let ws = dm.service("Workspace").unwrap();
        let p = dm.create("Part", Some(ws)).unwrap();
        dm.set_prop(p, "Name", Val::Str("Baseplate".into())).unwrap();
        assert_eq!(dm.full_name(p), "Workspace.Baseplate");
        assert_eq!(dm.get_prop(p, "Size"), Some(&Val::Arr(vec![4.0, 1.2, 2.0])));
        let q = dm.create("Part", Some(p)).unwrap();
        assert!(dm.set_parent(p, Some(q)).is_err());
        let c = dm.clone_tree(p).unwrap();
        assert_eq!(dm.get(c).unwrap().children.len(), 1);
        dm.destroy(p);
        assert!(dm.find_child(ws, "Baseplate", false).is_none());
    }

    #[test]
    fn load_place() {
        let b = match std::fs::read("/var/www/assets/assets/1939") {
            Ok(b) => b,
            Err(_) => return,
        };
        let tree = crate::rbx::parse(&b).unwrap();
        let mut dm = DataModel::new("server");
        let root = dm.root;
        let ids = dm.load_tree(&tree, root);
        assert!(!ids.is_empty());
        let ws = dm.service("Workspace").unwrap();
        let mut all = vec![];
        dm.descendants(ws, &mut all);
        assert!(!all.is_empty());
    }
}
