---
name: render-parity
description: Run the vanilla PHP versus Swirl render comparison and read the pixel diff. Use when checking Swirl renderer output or after changing swirl-render.
---

# Render parity

`tools/parity/run.sh <name> [options]` renders the same request with the frozen PHP reference (`tools/parity/php_render.php`, in `tools/parity/php-ref`, read-only fetcher, throwaway cache dir) and with `bin/swirl`, then runs `bin/swirl diff` on the two PNGs.

- Set `PARITY_EXACT=1` to also diff against PHP exact mode.
- Options: `--ids=1,2 --colors=194,... --size=352x352 --view=body|head --rig=R6 --stance=normal`.
- Outputs: `/tmp/swirl/<name>.php.png`, `/tmp/swirl/<name>.swirl.png`, `/tmp/swirl/<name>.diff.png`.
- The diff prints mean absolute error per channel, the percentage of pixels differing by more than 8, the max difference and the alpha mismatch count. Look at the PNGs with the Read tool.
- PHP renders with the fast rasteriser by default; `--exact=1` uses the reference one. Swirl follows the reference one, so compare against both when the numbers are off.
- Never point the harness at the live cache or write to /var/www/assets.
- Record results per case in PLAN.md.

- `bin/swirl` takes `--ids`, `--item` (with `--with`), `--colors` (brick numbers or #rrggbb), `--size`, `--view`, `--rig=R15`, `--scale=height:0.9,width:0.8,head:0.95`, `--pose`, `--stance`, `--straight-legs`, `--exact`. Exit 3 means an item error, 4 an unsupported asset format, 5 an asset missing from local disk; Solar keeps the old image and logs on any non-zero exit.
- PHP default mode uses the fast rasteriser, which Swirl ports; `--exact` plus `PARITY_EXACT=1` compares the reference rasteriser.
