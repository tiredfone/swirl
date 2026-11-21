---
name: subagent-tasks
description: Hand Swirl work to subagents (Claude Sonnet) to save usage. Use when asked to start an agent or subagent, or when a Swirl task is self-contained enough to delegate.
---

# Delegating Swirl work to subagents

The main session plans, checks results and talks to the owner. Subagents read, write Rust and test.

## When to delegate

- One crate or module port with a clear parity check (for example a mesh reader, the rasteriser, the rbxm reader).
- Not worth it: one-line fixes, or anything that needs the owner's decision first.

## How to start one

Use the Agent tool with `model: "sonnet"` and `run_in_background: true`. The prompt must stand alone and give:

1. Where and what: "Swirl at /var/www/swirl, a Rust rewrite of Chocolate (/var/www/html/chocolate) and vanilla (/var/www/html/core/vanilla-rcc)." Tell it to read /var/www/swirl/CLAUDE.md and PLAN.md first, and `export PATH=$HOME/.cargo/bin:$PATH`.
2. The rules: no code comments, never print secrets or read core/config.php, do not modify /var/www/html or /var/www/assets, do not touch ports 4201 and 4202 or the database, read-only fetches only.
3. The task: the module, the source files, what is already ported, the PLAN.md row.
4. How to verify: `cargo test`, the `render-parity` skill, PNGs under /tmp/swirl viewed with the Read tool.
5. Other agents running: name the files they edit; re-read before editing.
6. The report: what was ported, parity numbers, files changed, what is left. Tell it to commit by file name in /var/www/swirl and not push.

## Keeping usage down

- In the prompt: "Read only the parts of files you need (grep -n, sed -n ranges), don't dump big outputs, don't re-read files you just edited."
- Never read an agent's output file; its report arrives as a message.
- Don't repeat an agent's work in the main session while it runs, and don't poll it.
- Relay its report to the owner in a short summary; don't paste it.
- Start a fresh agent for a new problem instead of resuming an old one with stale context, unless the follow-up is about its own work.
- Don't spawn an agent to read a few lines you could read yourself.
- After it reports, spot-check only what matters (for example `php -l`, or one render), not the whole job.

## After it reports

- Spot-check one parity run and `cargo test`.
- Tell the owner what is done, what is partly done, and any side effects.
- Update PLAN.md and push (never force).
