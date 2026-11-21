---
name: render-parity
description: Run the vanilla PHP versus Swirl render comparison and read the pixel diff. Use when checking Swirl renderer output or after changing swirl-render.
---

# Render parity

`tools/parity/run.sh <name> [options]` renders the same request with PHP vanilla (`tools/parity/php_render.php`, read-only fetcher, throwaway cache dir) and with `swirl-vanilla`, then runs `swirl-vanilla diff` on the two PNGs.

- Set `PARITY_EXACT=1` to also diff against PHP exact mode.
- Options: `--ids=1,2 --colors=194,... --size=352x352 --view=body|head --rig=R6 --stance=normal`.
- Outputs: `/tmp/swirl/<name>.php.png`, `/tmp/swirl/<name>.swirl.png`, `/tmp/swirl/<name>.diff.png`.
- The diff prints mean absolute error per channel, the percentage of pixels differing by more than 8, the max difference and the alpha mismatch count. Look at the PNGs with the Read tool.
- PHP renders with the fast rasteriser by default; `--exact=1` uses the reference one. Swirl follows the reference one, so compare against both when the numbers are off.
- Never point the harness at the live cache or write to /var/www/assets.
- Record results per case in PLAN.md.
