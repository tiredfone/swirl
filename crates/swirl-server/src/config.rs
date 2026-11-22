use std::collections::HashMap;

pub fn parse_php_array(src: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let b: Vec<char> = src.chars().collect();
    let mut i = 0;
    let mut in_array = false;
    while i < b.len() {
        let c = b[i];
        if !in_array {
            if c == '[' {
                in_array = true;
            }
            i += 1;
            continue;
        }
        if c == '\'' || c == '"' {
            let (key, ni) = read_str(&b, i);
            i = ni;
            while i < b.len() && b[i].is_whitespace() {
                i += 1;
            }
            if i + 1 < b.len() && b[i] == '=' && b[i + 1] == '>' {
                i += 2;
                while i < b.len() && b[i].is_whitespace() {
                    i += 1;
                }
                if i < b.len() && (b[i] == '\'' || b[i] == '"') {
                    let (val, ni) = read_str(&b, i);
                    i = ni;
                    out.insert(key, val);
                } else {
                    let st = i;
                    while i < b.len() && b[i] != ',' && b[i] != ']' && b[i] != '\n' {
                        i += 1;
                    }
                    let raw: String = b[st..i].iter().collect();
                    out.insert(key, raw.trim().to_string());
                }
            }
            continue;
        }
        i += 1;
    }
    out
}

fn read_str(b: &[char], start: usize) -> (String, usize) {
    let q = b[start];
    let mut i = start + 1;
    let mut s = String::new();
    while i < b.len() && b[i] != q {
        if b[i] == '\\' && i + 1 < b.len() && (b[i + 1] == q || b[i + 1] == '\\') {
            s.push(b[i + 1]);
            i += 2;
            continue;
        }
        s.push(b[i]);
        i += 1;
    }
    (s, i + 1)
}

fn repo_root() -> std::path::PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        let mut d = exe.parent().map(|p| p.to_path_buf());
        while let Some(p) = d {
            if p.join("data/server").is_dir() {
                return p;
            }
            d = p.parent().map(|x| x.to_path_buf());
        }
    }
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

pub fn default_config_path() -> String {
    std::env::var("SWIRL_CONFIG").unwrap_or_else(|_| repo_root().join("config.local.php").to_string_lossy().to_string())
}

pub fn default_data_dir() -> String {
    std::env::var("SWIRL_DATA").unwrap_or_else(|_| repo_root().join("data/server").to_string_lossy().to_string())
}

pub fn default_store_dir() -> String {
    std::env::var("SWIRL_STORE").unwrap_or_else(|_| repo_root().join("data/store").to_string_lossy().to_string())
}

#[derive(Default, Clone)]
pub struct Config {
    pub map: HashMap<String, String>,
}

impl Config {
    pub fn load(path: &str) -> Config {
        match std::fs::read_to_string(path) {
            Ok(s) => Config { map: parse_php_array(&s) },
            Err(_) => Config::default(),
        }
    }

    pub fn get(&self, k: &str) -> Option<String> {
        self.map.get(k).filter(|v| !v.is_empty()).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_example() {
        let m = parse_php_array("<?php\nreturn [\n    \"a\" => \"http://x/y\",\n    'k' => 'q\\'z',\n    \"e\" => \"\",\n    'n' => 5,\n];\n");
        assert_eq!(m["a"], "http://x/y");
        assert_eq!(m["k"], "q'z");
        assert_eq!(m["e"], "");
        assert_eq!(m["n"], "5");
    }
}
