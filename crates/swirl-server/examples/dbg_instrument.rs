fn main() {
    let a: Vec<String> = std::env::args().collect();
    let src = std::fs::read_to_string(&a[1]).unwrap();
    print!("{}", swirl_server::debugger::instrument(&src, "S1"));
}
