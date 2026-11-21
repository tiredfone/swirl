fn main() {
    println!("cargo:rerun-if-changed=data/runtime.lua");
    println!("cargo:rerun-if-changed=/var/www/html/chocolate/shared/runtime.lua");
    let ours = std::fs::read("data/runtime.lua").expect("data/runtime.lua");
    if let Ok(theirs) = std::fs::read("/var/www/html/chocolate/shared/runtime.lua") {
        if ours != theirs {
            panic!("crates/swirl-lua/data/runtime.lua differs from /var/www/html/chocolate/shared/runtime.lua; run tools/sync-runtime.sh");
        }
    }
}
