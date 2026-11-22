fn main() {
    println!("cargo:rerun-if-changed=data/runtime.lua");
    println!("cargo:rerun-if-changed=data/runtime2018.lua");
    println!("cargo:rerun-if-changed=data/luadata.lua");
}
