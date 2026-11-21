fn main() {
    println!("cargo:rerun-if-changed=data/runtime.lua");
    println!("cargo:rerun-if-changed=/var/www/html/chocolate/shared/runtime.lua");
    println!("cargo:rerun-if-changed=data/runtime2018.lua");
    println!("cargo:rerun-if-changed=/var/www/html/chocolate/shared/runtime2018.lua");
    let ours2 = std::fs::read("data/runtime2018.lua").expect("data/runtime2018.lua");
    if let Ok(theirs) = std::fs::read("/var/www/html/chocolate/shared/runtime2018.lua") {
        if ours2 != theirs {
            panic!("crates/swirl-lua/data/runtime2018.lua differs from Chocolate's; run tools/sync-runtime.sh");
        }
    }
    let ours = std::fs::read("data/runtime.lua").expect("data/runtime.lua");
    if let Ok(theirs) = std::fs::read("/var/www/html/chocolate/shared/runtime.lua") {
        if ours != theirs {
            panic!("crates/swirl-lua/data/runtime.lua differs from /var/www/html/chocolate/shared/runtime.lua; run tools/sync-runtime.sh");
        }
    }
}
