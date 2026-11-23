import fs from 'node:fs';

const file = process.argv[2];
let s = fs.readFileSync(file, 'utf8');
s = s.replace(/^import \* as import\d+ from "wasi_snapshot_preview1"\n/gm, '');
s = s.replace(/^\s*"wasi_snapshot_preview1": import\d+,\n/gm, '');
s = s.replace('        "./swirl_client_bg.js": import0,', '        "./swirl_client_bg.js": import0,\n        "wasi_snapshot_preview1": swirlWasi(),');
const shim = `
function swirlWasi() {
    const view = () => new DataView(wasm.memory.buffer);
    return {
        environ_sizes_get: (count, size) => { view().setUint32(count, 0, true); view().setUint32(size, 0, true); return 0; },
        environ_get: () => 0,
        clock_time_get: (id, precision, out) => { view().setBigUint64(out, BigInt(Math.round(performance.now() * 1e6)), true); return 0; },
        fd_write: (fd, iovs, n, out) => {
            let total = 0;
            for (let i = 0; i < n; i++) total += view().getUint32(iovs + i * 8 + 4, true);
            view().setUint32(out, total, true);
            return 0;
        },
        fd_close: () => 8,
        fd_fdstat_get: () => 8,
        fd_fdstat_set_flags: () => 8,
        fd_prestat_get: () => 8,
        fd_prestat_dir_name: () => 8,
        fd_read: () => 8,
        fd_renumber: () => 8,
        fd_seek: () => 8,
        path_open: () => 8,
        path_remove_directory: () => 8,
        path_rename: () => 8,
        path_unlink_file: () => 8,
        proc_exit: (code) => { throw new Error('exit ' + code); },
    };
}
`;
s = s.replace('function __wbg_get_imports() {', shim + '\nfunction __wbg_get_imports() {');
fs.writeFileSync(file, s);
