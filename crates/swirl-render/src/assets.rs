use std::path::PathBuf;

pub trait AssetSource {
    fn fetch(&self, id: &str) -> Option<Vec<u8>>;
}

pub struct DirAssets {
    pub dirs: Vec<PathBuf>,
}

impl DirAssets {
    pub fn solar(root: &str) -> DirAssets {
        DirAssets { dirs: vec![PathBuf::from(format!("{}/assets", root)), PathBuf::from(format!("{}/cache", root))] }
    }
}

impl AssetSource for DirAssets {
    fn fetch(&self, id: &str) -> Option<Vec<u8>> {
        if id.is_empty() || !id.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let n: u64 = id.parse().ok()?;
        for d in &self.dirs {
            if let Ok(b) = std::fs::read(d.join(n.to_string())) {
                if !b.is_empty() {
                    return Some(b);
                }
            }
        }
        None
    }
}
