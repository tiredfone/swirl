---
name: port-module
description: Port a Chocolate (JS) or vanilla (PHP) file to a Swirl Rust crate and check parity. Use when asked to port, translate or rewrite a module into Swirl.
---

# Porting a module

1. Find the file in the mapping table in `/var/www/swirl/PLAN.md` to get its target crate and module.
2. Read only the needed parts of the source (`grep -n`, `sed -n`). Port in the same order and with the same arithmetic (f64, same rounding such as `(int)round()` and `(int)($x + 0.5)`); PHP `round` rounds half away from zero, which is `f64::round`.
3. No comments in code. No unsafe unless a profile shows it is needed.
4. Add a test next to the module that loads real data read-only from `/var/www/assets` or `/var/www/assets/render-data`.
5. For renderer work, run the `render-parity` skill and record the numbers in PLAN.md.
6. For core or Lua work, compare against Chocolate's output on the same file (dump the tree from node, diff with the Rust dump).
7. Update PLAN.md status, `cargo build` and `cargo test`, commit by file name.
