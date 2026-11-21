# Swirl

Rust rewrite of Chocolate (browser game client and server) and vanilla (software avatar renderer) for Solar. See PLAN.md for the module mapping, port order and parity status, and CLAUDE.md for the rules.

    export PATH=$HOME/.cargo/bin:$PATH
    cargo build --release
    ./target/release/swirl-vanilla --ids=1005,1011 --colors=24,23,24,24,119,119 --out=/tmp/swirl/a.png
    tools/parity/run.sh hat1 --ids=1005

Crates: swirl-core (math, model readers), swirl-render (renderer), swirl-vanilla (CLI), swirl-physics, swirl-lua, swirl-server, swirl-client (stubs).
