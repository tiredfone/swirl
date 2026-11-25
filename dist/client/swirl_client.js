

export class Boot {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        BootFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_boot_free(ptr, 0);
    }
    fail() {
        wasm.boot_fail(this.__wbg_ptr);
    }
    /**
     * @param {number} era
     * @param {number} bricks
     * @param {number} connectors
     * @param {number} now
     */
    finish(era, bricks, connectors, now) {
        wasm.boot_finish(this.__wbg_ptr, era, bricks, connectors, now);
    }
    constructor() {
        const ret = wasm.boot_new();
        this.__wbg_ptr = ret;
        BootFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * @returns {boolean}
     */
    present() {
        const ret = wasm.boot_present(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * @param {string} text
     */
    set_status(text) {
        const ptr0 = passStringToWasm0(text, wasm.__wbindgen_malloc_command_export, wasm.__wbindgen_realloc_command_export);
        const len0 = WASM_VECTOR_LEN;
        wasm.boot_set_status(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * @param {number} now
     * @returns {boolean}
     */
    step(now) {
        const ret = wasm.boot_step(this.__wbg_ptr, now);
        return ret !== 0;
    }
}
if (Symbol.dispose) Boot.prototype[Symbol.dispose] = Boot.prototype.free;

export class Client {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        ClientFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_client_free(ptr, 0);
    }
    /**
     * @param {string} text
     */
    apply_local(text) {
        const ptr0 = passStringToWasm0(text, wasm.__wbindgen_malloc_command_export, wasm.__wbindgen_realloc_command_export);
        const len0 = WASM_VECTOR_LEN;
        wasm.client_apply_local(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * @returns {Uint32Array}
     */
    boot_counts() {
        const ret = wasm.client_boot_counts(this.__wbg_ptr);
        var v1 = getArrayU32FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free_command_export(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * @returns {string}
     */
    camera_pose() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.client_camera_pose(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free_command_export(deferred1_0, deferred1_1, 1);
        }
    }
    clear_focus() {
        wasm.client_clear_focus(this.__wbg_ptr);
    }
    /**
     * @param {string} key
     */
    clear_terrain_chunk(key) {
        const ptr0 = passStringToWasm0(key, wasm.__wbindgen_malloc_command_export, wasm.__wbindgen_realloc_command_export);
        const len0 = WASM_VECTOR_LEN;
        wasm.client_clear_terrain_chunk(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * @param {string} url
     */
    connect(url) {
        const ptr0 = passStringToWasm0(url, wasm.__wbindgen_malloc_command_export, wasm.__wbindgen_realloc_command_export);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.client_connect(this.__wbg_ptr, ptr0, len0);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * @returns {string}
     */
    debug() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.client_debug(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free_command_export(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @returns {Uint8Array}
     */
    debug_atlas() {
        const ret = wasm.client_debug_atlas(this.__wbg_ptr);
        var v1 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free_command_export(ret[0], ret[1] * 1, 1);
        return v1;
    }
    /**
     * @returns {string}
     */
    debug_parts() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.client_debug_parts(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free_command_export(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @returns {number}
     */
    era() {
        const ret = wasm.client_era(this.__wbg_ptr);
        return ret;
    }
    /**
     * @param {number} x
     * @param {number} y
     * @param {number} z
     * @param {number} dist
     */
    focus(x, y, z, dist) {
        wasm.client_focus(this.__wbg_ptr, x, y, z, dist);
    }
    /**
     * @param {number} now
     */
    frame(now) {
        wasm.client_frame(this.__wbg_ptr, now);
    }
    /**
     * @returns {string}
     */
    inventory() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.client_inventory(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free_command_export(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @returns {number}
     */
    loaded_meshes() {
        const ret = wasm.client_loaded_meshes(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {number}
     */
    loaded_textures() {
        const ret = wasm.client_loaded_textures(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @param {HTMLCanvasElement} canvas
     */
    constructor(canvas) {
        const ret = wasm.client_new(canvas);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        this.__wbg_ptr = ret[0];
        ClientFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * @returns {string}
     */
    own_pos() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.client_own_pos(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free_command_export(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @returns {number}
     */
    part_count() {
        const ret = wasm.client_part_count(this.__wbg_ptr);
        return ret >>> 0;
    }
    ping() {
        wasm.client_ping(this.__wbg_ptr);
    }
    /**
     * @param {string} kind
     * @param {string} id
     */
    provide_failed(kind, id) {
        const ptr0 = passStringToWasm0(kind, wasm.__wbindgen_malloc_command_export, wasm.__wbindgen_realloc_command_export);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(id, wasm.__wbindgen_malloc_command_export, wasm.__wbindgen_realloc_command_export);
        const len1 = WASM_VECTOR_LEN;
        wasm.client_provide_failed(this.__wbg_ptr, ptr0, len0, ptr1, len1);
    }
    /**
     * @param {string} id
     * @param {Uint8Array} bytes
     */
    provide_mesh(id, bytes) {
        const ptr0 = passStringToWasm0(id, wasm.__wbindgen_malloc_command_export, wasm.__wbindgen_realloc_command_export);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passArray8ToWasm0(bytes, wasm.__wbindgen_malloc_command_export);
        const len1 = WASM_VECTOR_LEN;
        wasm.client_provide_mesh(this.__wbg_ptr, ptr0, len0, ptr1, len1);
    }
    /**
     * @param {string} id
     * @param {ImageBitmap} img
     */
    provide_texture(id, img) {
        const ptr0 = passStringToWasm0(id, wasm.__wbindgen_malloc_command_export, wasm.__wbindgen_realloc_command_export);
        const len0 = WASM_VECTOR_LEN;
        wasm.client_provide_texture(this.__wbg_ptr, ptr0, len0, img);
    }
    /**
     * @returns {boolean}
     */
    ready() {
        const ret = wasm.client_ready(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * @param {string} text
     */
    send(text) {
        const ptr0 = passStringToWasm0(text, wasm.__wbindgen_malloc_command_export, wasm.__wbindgen_realloc_command_export);
        const len0 = WASM_VECTOR_LEN;
        wasm.client_send(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * @param {Float32Array} cf
     * @param {number} fov_deg
     */
    set_camera(cf, fov_deg) {
        const ptr0 = passArrayF32ToWasm0(cf, wasm.__wbindgen_malloc_command_export);
        const len0 = WASM_VECTOR_LEN;
        wasm.client_set_camera(this.__wbg_ptr, ptr0, len0, fov_deg);
    }
    /**
     * @param {Float64Array} data
     */
    set_cfs(data) {
        const ptr0 = passArrayF64ToWasm0(data, wasm.__wbindgen_malloc_command_export);
        const len0 = WASM_VECTOR_LEN;
        wasm.client_set_cfs(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * @param {boolean} on
     */
    set_driven(on) {
        wasm.client_set_driven(this.__wbg_ptr, on);
    }
    /**
     * @param {number} id
     * @param {number} t
     */
    set_local_alpha(id, t) {
        wasm.client_set_local_alpha(this.__wbg_ptr, id, t);
    }
    /**
     * @param {Array<any>} faces
     */
    set_sky_cube(faces) {
        wasm.client_set_sky_cube(this.__wbg_ptr, faces);
    }
    /**
     * @param {string} key
     * @param {Float32Array} pos
     * @param {Float32Array} nrm
     * @param {Float32Array} uv
     * @param {Float32Array} info
     * @param {string} kind
     */
    set_terrain_chunk(key, pos, nrm, uv, info, kind) {
        const ptr0 = passStringToWasm0(key, wasm.__wbindgen_malloc_command_export, wasm.__wbindgen_realloc_command_export);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passArrayF32ToWasm0(pos, wasm.__wbindgen_malloc_command_export);
        const len1 = WASM_VECTOR_LEN;
        const ptr2 = passArrayF32ToWasm0(nrm, wasm.__wbindgen_malloc_command_export);
        const len2 = WASM_VECTOR_LEN;
        const ptr3 = passArrayF32ToWasm0(uv, wasm.__wbindgen_malloc_command_export);
        const len3 = WASM_VECTOR_LEN;
        const ptr4 = passArrayF32ToWasm0(info, wasm.__wbindgen_malloc_command_export);
        const len4 = WASM_VECTOR_LEN;
        const ptr5 = passStringToWasm0(kind, wasm.__wbindgen_malloc_command_export, wasm.__wbindgen_realloc_command_export);
        const len5 = WASM_VECTOR_LEN;
        wasm.client_set_terrain_chunk(this.__wbg_ptr, ptr0, len0, ptr1, len1, ptr2, len2, ptr3, len3, ptr4, len4, ptr5, len5);
    }
    /**
     * @returns {string}
     */
    status() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.client_status(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free_command_export(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @returns {string}
     */
    take_remote() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.client_take_remote(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free_command_export(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @returns {string}
     */
    take_requests() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.client_take_requests(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free_command_export(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @returns {string}
     */
    take_tap() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.client_take_tap(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free_command_export(deferred1_0, deferred1_1, 1);
        }
    }
}
if (Symbol.dispose) Client.prototype[Symbol.dispose] = Client.prototype.free;

export class DevConsole {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        DevConsoleFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_devconsole_free(ptr, 0);
    }
    /**
     * @param {string} source
     * @param {string} kind
     * @param {string} msg
     */
    add(source, kind, msg) {
        const ptr0 = passStringToWasm0(source, wasm.__wbindgen_malloc_command_export, wasm.__wbindgen_realloc_command_export);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(kind, wasm.__wbindgen_malloc_command_export, wasm.__wbindgen_realloc_command_export);
        const len1 = WASM_VECTOR_LEN;
        const ptr2 = passStringToWasm0(msg, wasm.__wbindgen_malloc_command_export, wasm.__wbindgen_realloc_command_export);
        const len2 = WASM_VECTOR_LEN;
        wasm.devconsole_add(this.__wbg_ptr, ptr0, len0, ptr1, len1, ptr2, len2);
    }
    /**
     * @returns {boolean}
     */
    is_open() {
        const ret = wasm.devconsole_is_open(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * @param {Element} parent
     */
    constructor(parent) {
        const ret = wasm.devconsole_new(parent);
        this.__wbg_ptr = ret;
        DevConsoleFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    toggle() {
        wasm.devconsole_toggle(this.__wbg_ptr);
    }
}
if (Symbol.dispose) DevConsole.prototype[Symbol.dispose] = DevConsole.prototype.free;

export class LuaVm {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        LuaVmFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_luavm_free(ptr, 0);
    }
    /**
     * @param {string} name
     * @returns {string}
     */
    abs_of(name) {
        let deferred2_0;
        let deferred2_1;
        try {
            const ptr0 = passStringToWasm0(name, wasm.__wbindgen_malloc_command_export, wasm.__wbindgen_realloc_command_export);
            const len0 = WASM_VECTOR_LEN;
            const ret = wasm.luavm_abs_of(this.__wbg_ptr, ptr0, len0);
            deferred2_0 = ret[0];
            deferred2_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free_command_export(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * @returns {string}
     */
    anim_pose() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.luavm_anim_pose(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free_command_export(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @param {string} text
     * @returns {boolean}
     */
    apply(text) {
        const ptr0 = passStringToWasm0(text, wasm.__wbindgen_malloc_command_export, wasm.__wbindgen_realloc_command_export);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.luavm_apply(this.__wbg_ptr, ptr0, len0);
        return ret !== 0;
    }
    /**
     * @param {HTMLElement} root
     */
    attach_gui(root) {
        wasm.luavm_attach_gui(this.__wbg_ptr, root);
    }
    /**
     * @returns {string}
     */
    backpack_items() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.luavm_backpack_items(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free_command_export(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @param {number} dx
     * @param {number} dy
     */
    cam_drag(dx, dy) {
        wasm.luavm_cam_drag(this.__wbg_ptr, dx, dy);
    }
    /**
     * @returns {boolean}
     */
    cam_first_person() {
        const ret = wasm.luavm_cam_first_person(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * @param {number} v
     */
    cam_key_yaw(v) {
        wasm.luavm_cam_key_yaw(this.__wbg_ptr, v);
    }
    /**
     * @param {number} dx
     * @param {number} dy
     */
    cam_rotate(dx, dy) {
        wasm.luavm_cam_rotate(this.__wbg_ptr, dx, dy);
    }
    /**
     * @returns {string}
     */
    cam_state() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.luavm_cam_state(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free_command_export(deferred1_0, deferred1_1, 1);
        }
    }
    cam_toggle_shift_lock() {
        wasm.luavm_cam_toggle_shift_lock(this.__wbg_ptr);
    }
    /**
     * @param {number} dir
     */
    cam_wheel(dir) {
        wasm.luavm_cam_wheel(this.__wbg_ptr, dir);
    }
    /**
     * @param {number} f
     */
    cam_zoom_by(f) {
        wasm.luavm_cam_zoom_by(this.__wbg_ptr, f);
    }
    /**
     * @param {number} yaw
     * @param {number} pitch
     * @param {number} zoom
     * @param {boolean} shift_lock
     * @param {boolean} pointer_locked
     * @param {number} key_yaw
     * @param {number} dt
     * @returns {string}
     */
    camera_update(yaw, pitch, zoom, shift_lock, pointer_locked, key_yaw, dt) {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.luavm_camera_update(this.__wbg_ptr, yaw, pitch, zoom, shift_lock, pointer_locked, key_yaw, dt);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free_command_export(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @param {string} path
     * @returns {string}
     */
    children_of(path) {
        let deferred2_0;
        let deferred2_1;
        try {
            const ptr0 = passStringToWasm0(path, wasm.__wbindgen_malloc_command_export, wasm.__wbindgen_realloc_command_export);
            const len0 = WASM_VECTOR_LEN;
            const ret = wasm.luavm_children_of(this.__wbg_ptr, ptr0, len0);
            deferred2_0 = ret[0];
            deferred2_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free_command_export(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * @param {string} root_name
     * @returns {string}
     */
    class_counts(root_name) {
        let deferred2_0;
        let deferred2_1;
        try {
            const ptr0 = passStringToWasm0(root_name, wasm.__wbindgen_malloc_command_export, wasm.__wbindgen_realloc_command_export);
            const len0 = WASM_VECTOR_LEN;
            const ret = wasm.luavm_class_counts(this.__wbg_ptr, ptr0, len0);
            deferred2_0 = ret[0];
            deferred2_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free_command_export(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * @returns {number}
     */
    client_era() {
        const ret = wasm.luavm_client_era(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {string}
     */
    debug_state() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.luavm_debug_state(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free_command_export(deferred1_0, deferred1_1, 1);
        }
    }
    drop_tool() {
        wasm.luavm_drop_tool(this.__wbg_ptr);
    }
    /**
     * @param {string} name
     * @returns {string}
     */
    dump_named(name) {
        let deferred2_0;
        let deferred2_1;
        try {
            const ptr0 = passStringToWasm0(name, wasm.__wbindgen_malloc_command_export, wasm.__wbindgen_realloc_command_export);
            const len0 = WASM_VECTOR_LEN;
            const ret = wasm.luavm_dump_named(this.__wbg_ptr, ptr0, len0);
            deferred2_0 = ret[0];
            deferred2_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free_command_export(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * @param {string} service
     * @param {string} event
     * @param {string} args
     */
    fire_service(service, event, args) {
        const ptr0 = passStringToWasm0(service, wasm.__wbindgen_malloc_command_export, wasm.__wbindgen_realloc_command_export);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(event, wasm.__wbindgen_malloc_command_export, wasm.__wbindgen_realloc_command_export);
        const len1 = WASM_VECTOR_LEN;
        const ptr2 = passStringToWasm0(args, wasm.__wbindgen_malloc_command_export, wasm.__wbindgen_realloc_command_export);
        const len2 = WASM_VECTOR_LEN;
        wasm.luavm_fire_service(this.__wbg_ptr, ptr0, len0, ptr1, len1, ptr2, len2);
    }
    /**
     * @param {number} inst
     * @param {boolean} on
     */
    focus_text_box(inst, on) {
        wasm.luavm_focus_text_box(this.__wbg_ptr, inst, on);
    }
    /**
     * @param {number} dt
     * @param {number} fw
     * @param {number} sd
     * @param {boolean} jump
     * @param {boolean} dismount
     * @param {boolean} pointer_locked
     */
    frame(dt, fw, sd, jump, dismount, pointer_locked) {
        wasm.luavm_frame(this.__wbg_ptr, dt, fw, sd, jump, dismount, pointer_locked);
    }
    /**
     * @param {number} vw
     * @param {number} vh
     * @param {number} k
     * @param {number} l
     * @param {number} t
     * @param {number} r
     * @param {number} b
     * @param {boolean} show
     * @param {number} now_ms
     * @returns {number}
     */
    gui_frame(vw, vh, k, l, t, r, b, show, now_ms) {
        const ret = wasm.luavm_gui_frame(this.__wbg_ptr, vw, vh, k, l, t, r, b, show, now_ms);
        return ret >>> 0;
    }
    /**
     * @param {string} key
     * @returns {boolean}
     */
    gui_key_claimed(key) {
        const ptr0 = passStringToWasm0(key, wasm.__wbindgen_malloc_command_export, wasm.__wbindgen_realloc_command_export);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.luavm_gui_key_claimed(this.__wbg_ptr, ptr0, len0);
        return ret !== 0;
    }
    /**
     * @returns {string}
     */
    gui_node_names() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.luavm_gui_node_names(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free_command_export(deferred1_0, deferred1_1, 1);
        }
    }
    input_blur() {
        wasm.luavm_input_blur(this.__wbg_ptr);
    }
    /**
     * @param {number} button
     * @param {boolean} down
     * @param {boolean} over_gui
     */
    input_button(button, down, over_gui) {
        wasm.luavm_input_button(this.__wbg_ptr, button, down, over_gui);
    }
    /**
     * @param {string} code
     * @param {boolean} down
     * @param {boolean} typing
     * @param {boolean} is_repeat
     */
    input_key(code, down, typing, is_repeat) {
        const ptr0 = passStringToWasm0(code, wasm.__wbindgen_malloc_command_export, wasm.__wbindgen_realloc_command_export);
        const len0 = WASM_VECTOR_LEN;
        wasm.luavm_input_key(this.__wbg_ptr, ptr0, len0, down, typing, is_repeat);
    }
    /**
     * @param {number} dx
     * @param {number} dy
     * @param {boolean} over_gui
     * @param {boolean} locked
     */
    input_move(dx, dy, over_gui, locked) {
        wasm.luavm_input_move(this.__wbg_ptr, dx, dy, over_gui, locked);
    }
    /**
     * @param {number} dir
     * @param {boolean} over_gui
     */
    input_wheel(dir, over_gui) {
        wasm.luavm_input_wheel(this.__wbg_ptr, dir, over_gui);
    }
    /**
     * @returns {number}
     */
    instance_count() {
        const ret = wasm.luavm_instance_count(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {boolean}
     */
    is_typing() {
        const ret = wasm.luavm_is_typing(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * @param {string} k
     * @param {boolean} down
     */
    key_event(k, down) {
        const ptr0 = passStringToWasm0(k, wasm.__wbindgen_malloc_command_export, wasm.__wbindgen_realloc_command_export);
        const len0 = WASM_VECTOR_LEN;
        wasm.luavm_key_event(this.__wbg_ptr, ptr0, len0, down);
    }
    /**
     * @returns {string}
     */
    logs() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.luavm_logs(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free_command_export(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @returns {string}
     */
    motor_angles() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.luavm_motor_angles(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free_command_export(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @param {number} button
     * @param {boolean} down
     */
    mouse_button(button, down) {
        wasm.luavm_mouse_button(this.__wbg_ptr, button, down);
    }
    /**
     * @returns {string}
     */
    mouse_cursor() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.luavm_mouse_cursor(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free_command_export(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @param {string} name
     */
    mouse_event(name) {
        const ptr0 = passStringToWasm0(name, wasm.__wbindgen_malloc_command_export, wasm.__wbindgen_realloc_command_export);
        const len0 = WASM_VECTOR_LEN;
        wasm.luavm_mouse_event(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * @param {number} x
     * @param {number} y
     * @param {number} w
     * @param {number} h
     * @param {number} k
     * @param {boolean} over_gui
     * @param {boolean} centered
     */
    mouse_frame(x, y, w, h, k, over_gui, centered) {
        wasm.luavm_mouse_frame(this.__wbg_ptr, x, y, w, h, k, over_gui, centered);
    }
    /**
     * @returns {string}
     */
    mouse_props() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.luavm_mouse_props(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free_command_export(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @param {string} service
     * @param {string} _class
     * @returns {string}
     */
    names_under(service, _class) {
        let deferred3_0;
        let deferred3_1;
        try {
            const ptr0 = passStringToWasm0(service, wasm.__wbindgen_malloc_command_export, wasm.__wbindgen_realloc_command_export);
            const len0 = WASM_VECTOR_LEN;
            const ptr1 = passStringToWasm0(_class, wasm.__wbindgen_malloc_command_export, wasm.__wbindgen_realloc_command_export);
            const len1 = WASM_VECTOR_LEN;
            const ret = wasm.luavm_names_under(this.__wbg_ptr, ptr0, len0, ptr1, len1);
            deferred3_0 = ret[0];
            deferred3_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free_command_export(deferred3_0, deferred3_1, 1);
        }
    }
    /**
     * @param {number} wall
     */
    constructor(wall) {
        const ret = wasm.luavm_new(wall);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        this.__wbg_ptr = ret[0];
        LuaVmFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * @returns {boolean}
     */
    own_ready() {
        const ret = wasm.luavm_own_ready(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * @returns {string}
     */
    own_state() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.luavm_own_state(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free_command_export(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @returns {string}
     */
    phys_parts() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.luavm_phys_parts(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free_command_export(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @param {number} x
     * @param {number} y
     * @returns {boolean}
     */
    pointer_over_gui(x, y) {
        const ret = wasm.luavm_pointer_over_gui(this.__wbg_ptr, x, y);
        return ret !== 0;
    }
    /**
     * @param {number} id
     * @param {Uint8Array | null} [bytes]
     */
    provide_anim(id, bytes) {
        var ptr0 = isLikeNone(bytes) ? 0 : passArray8ToWasm0(bytes, wasm.__wbindgen_malloc_command_export);
        var len0 = WASM_VECTOR_LEN;
        wasm.luavm_provide_anim(this.__wbg_ptr, id, ptr0, len0);
    }
    /**
     * @param {number} id
     * @param {string | null} [text]
     */
    provide_core_source(id, text) {
        var ptr0 = isLikeNone(text) ? 0 : passStringToWasm0(text, wasm.__wbindgen_malloc_command_export, wasm.__wbindgen_realloc_command_export);
        var len0 = WASM_VECTOR_LEN;
        wasm.luavm_provide_core_source(this.__wbg_ptr, id, ptr0, len0);
    }
    /**
     * @param {number} token
     * @param {Uint8Array | null} [bytes]
     */
    provide_model(token, bytes) {
        var ptr0 = isLikeNone(bytes) ? 0 : passArray8ToWasm0(bytes, wasm.__wbindgen_malloc_command_export);
        var len0 = WASM_VECTOR_LEN;
        wasm.luavm_provide_model(this.__wbg_ptr, token, ptr0, len0);
    }
    /**
     * @returns {string}
     */
    render_alpha() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.luavm_render_alpha(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free_command_export(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @returns {Float32Array}
     */
    render_camera() {
        const ret = wasm.luavm_render_camera(this.__wbg_ptr);
        var v1 = getArrayF32FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free_command_export(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * @returns {Float64Array}
     */
    render_cfs() {
        const ret = wasm.luavm_render_cfs(this.__wbg_ptr);
        var v1 = getArrayF64FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free_command_export(ret[0], ret[1] * 8, 8);
        return v1;
    }
    /**
     * @returns {string}
     */
    render_ops() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.luavm_render_ops(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free_command_export(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @param {string} source
     */
    run_script(source) {
        const ptr0 = passStringToWasm0(source, wasm.__wbindgen_malloc_command_export, wasm.__wbindgen_realloc_command_export);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.luavm_run_script(this.__wbg_ptr, ptr0, len0);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * @param {string} name
     * @returns {string}
     */
    script_source(name) {
        let deferred2_0;
        let deferred2_1;
        try {
            const ptr0 = passStringToWasm0(name, wasm.__wbindgen_malloc_command_export, wasm.__wbindgen_realloc_command_export);
            const len0 = WASM_VECTOR_LEN;
            const ret = wasm.luavm_script_source(this.__wbg_ptr, ptr0, len0);
            deferred2_0 = ret[0];
            deferred2_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free_command_export(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * @param {string} text
     */
    set_core_pack(text) {
        const ptr0 = passStringToWasm0(text, wasm.__wbindgen_malloc_command_export, wasm.__wbindgen_realloc_command_export);
        const len0 = WASM_VECTOR_LEN;
        wasm.luavm_set_core_pack(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * @param {boolean} forward
     * @param {boolean} back
     * @param {boolean} left
     * @param {boolean} right
     * @param {boolean} dismount
     */
    set_drive_input(forward, back, left, right, dismount) {
        wasm.luavm_set_drive_input(this.__wbg_ptr, forward, back, left, right, dismount);
    }
    /**
     * @param {string | null} [text]
     */
    set_gui_message(text) {
        var ptr0 = isLikeNone(text) ? 0 : passStringToWasm0(text, wasm.__wbindgen_malloc_command_export, wasm.__wbindgen_realloc_command_export);
        var len0 = WASM_VECTOR_LEN;
        wasm.luavm_set_gui_message(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * @param {number} x
     * @param {number} y
     * @param {boolean} on
     */
    set_gui_pointer(x, y, on) {
        wasm.luavm_set_gui_pointer(this.__wbg_ptr, x, y, on);
    }
    /**
     * @param {boolean} run
     */
    set_run(run) {
        wasm.luavm_set_run(this.__wbg_ptr, run);
    }
    /**
     * @param {number} w
     * @param {number} h
     */
    set_screen(w, h) {
        wasm.luavm_set_screen(this.__wbg_ptr, w, h);
    }
    /**
     * @param {string} head
     * @param {string} avatar
     */
    set_thumbnail_urls(head, avatar) {
        const ptr0 = passStringToWasm0(head, wasm.__wbindgen_malloc_command_export, wasm.__wbindgen_realloc_command_export);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(avatar, wasm.__wbindgen_malloc_command_export, wasm.__wbindgen_realloc_command_export);
        const len1 = WASM_VECTOR_LEN;
        wasm.luavm_set_thumbnail_urls(this.__wbg_ptr, ptr0, len0, ptr1, len1);
    }
    /**
     * @param {boolean} on
     */
    set_touch(on) {
        wasm.luavm_set_touch(this.__wbg_ptr, on);
    }
    /**
     * @returns {string}
     */
    sky_urls() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.luavm_sky_urls(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free_command_export(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @param {number} id
     */
    sound_ended(id) {
        wasm.luavm_sound_ended(this.__wbg_ptr, id);
    }
    /**
     * @param {number} now_ms
     */
    sync_now(now_ms) {
        wasm.luavm_sync_now(this.__wbg_ptr, now_ms);
    }
    /**
     * @returns {string | undefined}
     */
    take_kicked() {
        const ret = wasm.luavm_take_kicked(this.__wbg_ptr);
        let v1;
        if (ret[0] !== 0) {
            v1 = getStringFromWasm0(ret[0], ret[1]).slice();
            wasm.__wbindgen_free_command_export(ret[0], ret[1] * 1, 1);
        }
        return v1;
    }
    /**
     * @returns {boolean}
     */
    take_leave() {
        const ret = wasm.luavm_take_leave(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * @returns {number}
     */
    take_mouse_behavior() {
        const ret = wasm.luavm_take_mouse_behavior(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {string}
     */
    take_out() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.luavm_take_out(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free_command_export(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @returns {string}
     */
    take_sound_ops() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.luavm_take_sound_ops(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free_command_export(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @returns {string}
     */
    take_terrain_keys() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.luavm_take_terrain_keys(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free_command_export(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @returns {string}
     */
    take_wanted() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.luavm_take_wanted(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free_command_export(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @returns {string}
     */
    take_wanted_anims() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.luavm_take_wanted_anims(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free_command_export(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @returns {string}
     */
    take_wanted_models() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.luavm_take_wanted_models(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free_command_export(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @param {string} key
     * @returns {Array<any>}
     */
    terrain_chunk(key) {
        const ptr0 = passStringToWasm0(key, wasm.__wbindgen_malloc_command_export, wasm.__wbindgen_realloc_command_export);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.luavm_terrain_chunk(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @returns {number}
     */
    terrain_era() {
        const ret = wasm.luavm_terrain_era(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {string}
     */
    terrain_info() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.luavm_terrain_info(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free_command_export(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @returns {string}
     */
    text_bounds_all() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.luavm_text_bounds_all(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free_command_export(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @param {number} dt
     */
    tick(dt) {
        wasm.luavm_tick(this.__wbg_ptr, dt);
    }
    /**
     * @param {number} item
     */
    toggle_slot(item) {
        wasm.luavm_toggle_slot(this.__wbg_ptr, item);
    }
    /**
     * @returns {string}
     */
    tools_json() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.luavm_tools_json(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free_command_export(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @param {number} id
     * @param {number} x
     * @param {number} y
     * @param {number} k
     * @returns {boolean}
     */
    touch_down(id, x, y, k) {
        const ret = wasm.luavm_touch_down(this.__wbg_ptr, id, x, y, k);
        return ret !== 0;
    }
    /**
     * @param {number} x
     * @param {number} y
     * @param {number} k
     * @returns {string}
     */
    touch_hits(x, y, k) {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.luavm_touch_hits(this.__wbg_ptr, x, y, k);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free_command_export(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @param {number} id
     * @param {number} x
     * @param {number} y
     * @param {number} k
     * @returns {boolean}
     */
    touch_move(id, x, y, k) {
        const ret = wasm.luavm_touch_move(this.__wbg_ptr, id, x, y, k);
        return ret !== 0;
    }
    /**
     * @param {number} id
     * @param {number} x
     * @param {number} y
     * @param {number} k
     * @param {boolean} cancelled
     * @returns {boolean}
     */
    touch_up(id, x, y, k, cancelled) {
        const ret = wasm.luavm_touch_up(this.__wbg_ptr, id, x, y, k, cancelled);
        return ret !== 0;
    }
    /**
     * @param {number} dt
     * @param {number} mx
     * @param {number} mz
     * @param {boolean} jump
     */
    update(dt, mx, mz, jump) {
        wasm.luavm_update(this.__wbg_ptr, dt, mx, mz, jump);
    }
    /**
     * @param {number} ox
     * @param {number} oy
     * @param {number} oz
     * @param {number} dx
     * @param {number} dy
     * @param {number} dz
     * @param {number} x
     * @param {number} y
     * @param {boolean} over_gui
     * @param {number} k
     */
    update_mouse(ox, oy, oz, dx, dy, dz, x, y, over_gui, k) {
        wasm.luavm_update_mouse(this.__wbg_ptr, ox, oy, oz, dx, dy, dz, x, y, over_gui, k);
    }
    /**
     * @returns {string}
     */
    workspace_children() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.luavm_workspace_children(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free_command_export(deferred1_0, deferred1_1, 1);
        }
    }
}
if (Symbol.dispose) LuaVm.prototype[Symbol.dispose] = LuaVm.prototype.free;

export class Scene {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        SceneFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_scene_free(ptr, 0);
    }
    /**
     * @param {string} text
     * @returns {string}
     */
    apply(text) {
        let deferred2_0;
        let deferred2_1;
        try {
            const ptr0 = passStringToWasm0(text, wasm.__wbindgen_malloc_command_export, wasm.__wbindgen_realloc_command_export);
            const len0 = WASM_VECTOR_LEN;
            const ret = wasm.scene_apply(this.__wbg_ptr, ptr0, len0);
            deferred2_0 = ret[0];
            deferred2_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free_command_export(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * @returns {number}
     */
    era() {
        const ret = wasm.scene_era(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    instance_count() {
        const ret = wasm.scene_instance_count(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {number}
     */
    mesh_part_count() {
        const ret = wasm.scene_mesh_part_count(this.__wbg_ptr);
        return ret >>> 0;
    }
    constructor() {
        const ret = wasm.scene_new();
        this.__wbg_ptr = ret;
        SceneFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * @returns {number}
     */
    part_count() {
        const ret = wasm.scene_part_count(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {boolean}
     */
    ready() {
        const ret = wasm.scene_ready(this.__wbg_ptr);
        return ret !== 0;
    }
}
if (Symbol.dispose) Scene.prototype[Symbol.dispose] = Scene.prototype.free;

export class WebMeasure {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        WebMeasureFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_webmeasure_free(ptr, 0);
    }
}
if (Symbol.dispose) WebMeasure.prototype[Symbol.dispose] = WebMeasure.prototype.free;

/**
 * @returns {string}
 */
export function client_version() {
    let deferred1_0;
    let deferred1_1;
    try {
        const ret = wasm.client_version();
        deferred1_0 = ret[0];
        deferred1_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally {
        wasm.__wbindgen_free_command_export(deferred1_0, deferred1_1, 1);
    }
}

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

function __wbg_get_imports() {
    const import0 = {
        __proto__: null,
        __wbg___wbindgen_debug_string_edece8177ad01481: function(arg0, arg1) {
            const ret = debugString(arg1);
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc_command_export, wasm.__wbindgen_realloc_command_export);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg___wbindgen_is_function_5cd60d5cf78b4eef: function(arg0) {
            const ret = typeof(arg0) === 'function';
            return ret;
        },
        __wbg___wbindgen_is_undefined_35bb9f4c7fd651d5: function(arg0) {
            const ret = arg0 === undefined;
            return ret;
        },
        __wbg___wbindgen_number_get_f73a1244370fcc2c: function(arg0, arg1) {
            const obj = arg1;
            const ret = typeof(obj) === 'number' ? obj : undefined;
            getDataViewMemory0().setFloat64(arg0 + 8 * 1, isLikeNone(ret) ? 0 : ret, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, !isLikeNone(ret), true);
        },
        __wbg___wbindgen_string_get_d109740c0d18f4d7: function(arg0, arg1) {
            const obj = arg1;
            const ret = typeof(obj) === 'string' ? obj : undefined;
            var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_malloc_command_export, wasm.__wbindgen_realloc_command_export);
            var len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg___wbindgen_throw_9c31b086c2b26051: function(arg0, arg1) {
            throw new Error(getStringFromWasm0(arg0, arg1));
        },
        __wbg__wbg_cb_unref_3fa391f3fcdb55f8: function(arg0) {
            arg0._wbg_cb_unref();
        },
        __wbg_activeElement_63c3f3ce5e7ebe21: function(arg0) {
            const ret = arg0.activeElement;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_activeTexture_37cff0753870753b: function(arg0, arg1) {
            arg0.activeTexture(arg1 >>> 0);
        },
        __wbg_addEventListener_aedacff123afaebd: function() { return handleError(function (arg0, arg1, arg2, arg3) {
            arg0.addEventListener(getStringFromWasm0(arg1, arg2), arg3);
        }, arguments); },
        __wbg_add_ceec5fdac76a2ce5: function() { return handleError(function (arg0, arg1, arg2) {
            arg0.add(getStringFromWasm0(arg1, arg2));
        }, arguments); },
        __wbg_appendChild_6e88800a9a8fb360: function() { return handleError(function (arg0, arg1) {
            const ret = arg0.appendChild(arg1);
            return ret;
        }, arguments); },
        __wbg_attachShader_0a37c762590e5e1c: function(arg0, arg1, arg2) {
            arg0.attachShader(arg1, arg2);
        },
        __wbg_bindAttribLocation_07b2841d89fca977: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.bindAttribLocation(arg1, arg2 >>> 0, getStringFromWasm0(arg3, arg4));
        },
        __wbg_bindBuffer_1a31fd3809dc22c8: function(arg0, arg1, arg2) {
            arg0.bindBuffer(arg1 >>> 0, arg2);
        },
        __wbg_bindFramebuffer_751e5064f23ee1c4: function(arg0, arg1, arg2) {
            arg0.bindFramebuffer(arg1 >>> 0, arg2);
        },
        __wbg_bindSampler_708d9901a5e548b8: function(arg0, arg1, arg2) {
            arg0.bindSampler(arg1 >>> 0, arg2);
        },
        __wbg_bindTexture_7fd7f85d6f942f6f: function(arg0, arg1, arg2) {
            arg0.bindTexture(arg1 >>> 0, arg2);
        },
        __wbg_bindVertexArray_f8587a616356d307: function(arg0, arg1) {
            arg0.bindVertexArray(arg1);
        },
        __wbg_blendFuncSeparate_6f525092629a20ae: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.blendFuncSeparate(arg1 >>> 0, arg2 >>> 0, arg3 >>> 0, arg4 >>> 0);
        },
        __wbg_blur_2f5c68c34fbb5200: function() { return handleError(function (arg0) {
            arg0.blur();
        }, arguments); },
        __wbg_bottom_e7e0b0ae500f5673: function(arg0) {
            const ret = arg0.bottom;
            return ret;
        },
        __wbg_bufferData_74a0b79b4c9d8f96: function(arg0, arg1, arg2, arg3) {
            arg0.bufferData(arg1 >>> 0, arg2, arg3 >>> 0);
        },
        __wbg_button_61ec32cfadc0fbbe: function(arg0) {
            const ret = arg0.button;
            return ret;
        },
        __wbg_checked_eff4e633c5a4b381: function(arg0) {
            const ret = arg0.checked;
            return ret;
        },
        __wbg_childElementCount_863e15180da97ec2: function(arg0) {
            const ret = arg0.childElementCount;
            return ret;
        },
        __wbg_classList_8e2d38f6fb1576f2: function(arg0) {
            const ret = arg0.classList;
            return ret;
        },
        __wbg_clearColor_9b1cc35ab608d422: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.clearColor(arg1, arg2, arg3, arg4);
        },
        __wbg_clear_252bb7b11d5bea06: function(arg0, arg1) {
            arg0.clear(arg1 >>> 0);
        },
        __wbg_clientHeight_02b92bd8f52a1a32: function(arg0) {
            const ret = arg0.clientHeight;
            return ret;
        },
        __wbg_clientWidth_fa33fffa0a76d8ad: function(arg0) {
            const ret = arg0.clientWidth;
            return ret;
        },
        __wbg_clientX_4f65e02f53399dc1: function(arg0) {
            const ret = arg0.clientX;
            return ret;
        },
        __wbg_clientY_91dc5d824aabd324: function(arg0) {
            const ret = arg0.clientY;
            return ret;
        },
        __wbg_closest_e35efd6dd5eb01f3: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = arg0.closest(getStringFromWasm0(arg1, arg2));
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        }, arguments); },
        __wbg_code_98ceeaa5ff83fb0b: function(arg0) {
            const ret = arg0.code;
            return ret;
        },
        __wbg_compileShader_a20e7b68d3edcd8a: function(arg0, arg1) {
            arg0.compileShader(arg1);
        },
        __wbg_contains_7f528ae44500f46d: function(arg0, arg1) {
            const ret = arg0.contains(arg1);
            return ret;
        },
        __wbg_createBuffer_1c3448547584bc5a: function(arg0) {
            const ret = arg0.createBuffer();
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_createElement_d10771800cfb6e7e: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = arg0.createElement(getStringFromWasm0(arg1, arg2));
            return ret;
        }, arguments); },
        __wbg_createFramebuffer_22f50a7a9f8afdf0: function(arg0) {
            const ret = arg0.createFramebuffer();
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_createProgram_a175fc4c32429a24: function(arg0) {
            const ret = arg0.createProgram();
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_createSampler_80eb58b226692482: function(arg0) {
            const ret = arg0.createSampler();
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_createShader_9c5e52918428bd27: function(arg0, arg1) {
            const ret = arg0.createShader(arg1 >>> 0);
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_createTexture_5e721dc1ddd865e3: function(arg0) {
            const ret = arg0.createTexture();
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_createVertexArray_050d27763dfd72fa: function(arg0) {
            const ret = arg0.createVertexArray();
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_cullFace_632c5f88d252b4d7: function(arg0, arg1) {
            arg0.cullFace(arg1 >>> 0);
        },
        __wbg_data_5fc79a19e47d1531: function(arg0) {
            const ret = arg0.data;
            return ret;
        },
        __wbg_dataset_b1a207c85471bbee: function(arg0) {
            const ret = arg0.dataset;
            return ret;
        },
        __wbg_deltaY_9bad500402885525: function(arg0) {
            const ret = arg0.deltaY;
            return ret;
        },
        __wbg_depthMask_a00e4725581ef05d: function(arg0, arg1) {
            arg0.depthMask(arg1 !== 0);
        },
        __wbg_devicePixelRatio_5a86d9c1679c25c2: function(arg0) {
            const ret = arg0.devicePixelRatio;
            return ret;
        },
        __wbg_disable_df908054ffee7971: function(arg0, arg1) {
            arg0.disable(arg1 >>> 0);
        },
        __wbg_document_3540635616a18455: function(arg0) {
            const ret = arg0.document;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_drawArrays_7f9a3dcec5315ce5: function(arg0, arg1, arg2, arg3) {
            arg0.drawArrays(arg1 >>> 0, arg2, arg3);
        },
        __wbg_drawBuffers_217bd25bf75ccebd: function(arg0, arg1) {
            arg0.drawBuffers(arg1);
        },
        __wbg_drawImage_1b8a553b10e5592c: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5) {
            arg0.drawImage(arg1, arg2, arg3, arg4, arg5);
        }, arguments); },
        __wbg_drawImage_aee5d70fa2f0a7ed: function() { return handleError(function (arg0, arg1, arg2, arg3) {
            arg0.drawImage(arg1, arg2, arg3);
        }, arguments); },
        __wbg_drawImage_bf03a93112879523: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5) {
            arg0.drawImage(arg1, arg2, arg3, arg4, arg5);
        }, arguments); },
        __wbg_drawImage_c6659f999961906f: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9) {
            arg0.drawImage(arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9);
        }, arguments); },
        __wbg_elementFromPoint_fcf16f6ed18fb16b: function(arg0, arg1, arg2) {
            const ret = arg0.elementFromPoint(arg1, arg2);
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_elementsFromPoint_1cc8cdbe5dbe8117: function(arg0, arg1, arg2) {
            const ret = arg0.elementsFromPoint(arg1, arg2);
            return ret;
        },
        __wbg_enableVertexAttribArray_9963bb377f60317c: function(arg0, arg1) {
            arg0.enableVertexAttribArray(arg1 >>> 0);
        },
        __wbg_enable_5c8f846164bc8138: function(arg0, arg1) {
            arg0.enable(arg1 >>> 0);
        },
        __wbg_exitPointerLock_d5e12481fbb6ea3c: function(arg0) {
            arg0.exitPointerLock();
        },
        __wbg_fetch_872c7c4b806963cc: function(arg0, arg1, arg2) {
            const ret = arg0.fetch(getStringFromWasm0(arg1, arg2));
            return ret;
        },
        __wbg_fillRect_4f7134801b257e68: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.fillRect(arg1, arg2, arg3, arg4);
        },
        __wbg_firstElementChild_d534db2a0a86bd8a: function(arg0) {
            const ret = arg0.firstElementChild;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_focus_f37157dd6c795de6: function() { return handleError(function (arg0) {
            arg0.focus();
        }, arguments); },
        __wbg_framebufferTexture2D_91e307404924ae24: function(arg0, arg1, arg2, arg3, arg4, arg5) {
            arg0.framebufferTexture2D(arg1 >>> 0, arg2 >>> 0, arg3 >>> 0, arg4, arg5);
        },
        __wbg_generateMipmap_606b9590a2f77744: function(arg0, arg1) {
            arg0.generateMipmap(arg1 >>> 0);
        },
        __wbg_getAttribute_bea260b21f4e71cc: function(arg0, arg1, arg2, arg3) {
            const ret = arg1.getAttribute(getStringFromWasm0(arg2, arg3));
            var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_malloc_command_export, wasm.__wbindgen_realloc_command_export);
            var len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg_getBoundingClientRect_9169b7906daaa798: function(arg0) {
            const ret = arg0.getBoundingClientRect();
            return ret;
        },
        __wbg_getContext_32d5f94659d12566: function() { return handleError(function (arg0, arg1, arg2, arg3) {
            const ret = arg0.getContext(getStringFromWasm0(arg1, arg2), arg3);
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        }, arguments); },
        __wbg_getContext_47ea64e14d931e3e: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = arg0.getContext(getStringFromWasm0(arg1, arg2));
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        }, arguments); },
        __wbg_getContext_e1463ff7aa682d57: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = arg0.getContext(getStringFromWasm0(arg1, arg2));
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        }, arguments); },
        __wbg_getElementById_78449141d07cd8ef: function(arg0, arg1, arg2) {
            const ret = arg0.getElementById(getStringFromWasm0(arg1, arg2));
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_getExtension_c76ccfc25e343ce6: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = arg0.getExtension(getStringFromWasm0(arg1, arg2));
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        }, arguments); },
        __wbg_getHours_c974d920209733e8: function(arg0) {
            const ret = arg0.getHours();
            return ret;
        },
        __wbg_getItem_88cc26174f98c20c: function() { return handleError(function (arg0, arg1, arg2, arg3) {
            const ret = arg1.getItem(getStringFromWasm0(arg2, arg3));
            var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_malloc_command_export, wasm.__wbindgen_realloc_command_export);
            var len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        }, arguments); },
        __wbg_getMinutes_e2e8ae846b37b328: function(arg0) {
            const ret = arg0.getMinutes();
            return ret;
        },
        __wbg_getParameter_827c3142b1ce3364: function() { return handleError(function (arg0, arg1) {
            const ret = arg0.getParameter(arg1 >>> 0);
            return ret;
        }, arguments); },
        __wbg_getPropertyValue_fb7bf3ecc4f178c8: function() { return handleError(function (arg0, arg1, arg2, arg3) {
            const ret = arg1.getPropertyValue(getStringFromWasm0(arg2, arg3));
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc_command_export, wasm.__wbindgen_realloc_command_export);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        }, arguments); },
        __wbg_getSeconds_2782a558f414ec05: function(arg0) {
            const ret = arg0.getSeconds();
            return ret;
        },
        __wbg_getUniformLocation_46373021b59d8832: function(arg0, arg1, arg2, arg3) {
            const ret = arg0.getUniformLocation(arg1, getStringFromWasm0(arg2, arg3));
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_get_98fdf51d029a75eb: function(arg0, arg1) {
            const ret = arg0[arg1 >>> 0];
            return ret;
        },
        __wbg_height_aef2a2eb10d0d530: function(arg0) {
            const ret = arg0.height;
            return ret;
        },
        __wbg_instanceof_CanvasRenderingContext2d_d4be74cff7165c1e: function(arg0) {
            let result;
            try {
                result = arg0 instanceof CanvasRenderingContext2D;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_Element_244fd1e5d45219a6: function(arg0) {
            let result;
            try {
                result = arg0 instanceof Element;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_HtmlCanvasElement_a02da0a417f1bf3f: function(arg0) {
            let result;
            try {
                result = arg0 instanceof HTMLCanvasElement;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_HtmlElement_02c2813e0b28553a: function(arg0) {
            let result;
            try {
                result = arg0 instanceof HTMLElement;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_HtmlImageElement_491672dffc711dd2: function(arg0) {
            let result;
            try {
                result = arg0 instanceof HTMLImageElement;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_HtmlInputElement_4aa17180fd35ef36: function(arg0) {
            let result;
            try {
                result = arg0 instanceof HTMLInputElement;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_HtmlTextAreaElement_32557f7b0847ecd8: function(arg0) {
            let result;
            try {
                result = arg0 instanceof HTMLTextAreaElement;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_ImageBitmap_978cd0345c96f59d: function(arg0) {
            let result;
            try {
                result = arg0 instanceof ImageBitmap;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_KeyboardEvent_50cb8f33aac74605: function(arg0) {
            let result;
            try {
                result = arg0 instanceof KeyboardEvent;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_MouseEvent_9542b364a3982a8c: function(arg0) {
            let result;
            try {
                result = arg0 instanceof MouseEvent;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_OffscreenCanvasRenderingContext2d_0510c089e1f92f14: function(arg0) {
            let result;
            try {
                result = arg0 instanceof OffscreenCanvasRenderingContext2D;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_PointerEvent_5d5f7b56f1bb921b: function(arg0) {
            let result;
            try {
                result = arg0 instanceof PointerEvent;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_Response_ecfc823e8fb354e2: function(arg0) {
            let result;
            try {
                result = arg0 instanceof Response;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_WebGl2RenderingContext_419098f7bf88e87e: function(arg0) {
            let result;
            try {
                result = arg0 instanceof WebGL2RenderingContext;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_Window_faa5cf994f49cca7: function(arg0) {
            let result;
            try {
                result = arg0 instanceof Window;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_is_032c49d03f47f420: function(arg0, arg1) {
            const ret = Object.is(arg0, arg1);
            return ret;
        },
        __wbg_key_daba1c10e3b408ef: function(arg0, arg1) {
            const ret = arg1.key;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc_command_export, wasm.__wbindgen_realloc_command_export);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg_left_235f0dc266f4c6f7: function(arg0) {
            const ret = arg0.left;
            return ret;
        },
        __wbg_length_2591a0f4f659a55c: function(arg0) {
            const ret = arg0.length;
            return ret;
        },
        __wbg_linkProgram_7689cb555b14a359: function(arg0, arg1) {
            arg0.linkProgram(arg1);
        },
        __wbg_localStorage_e3f4a792bb36c514: function() { return handleError(function (arg0) {
            const ret = arg0.localStorage;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        }, arguments); },
        __wbg_measureText_3253bc5970b559a6: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = arg0.measureText(getStringFromWasm0(arg1, arg2));
            return ret;
        }, arguments); },
        __wbg_naturalHeight_4f0f7c830ee51fad: function(arg0) {
            const ret = arg0.naturalHeight;
            return ret;
        },
        __wbg_naturalWidth_bb168977ab076cca: function(arg0) {
            const ret = arg0.naturalWidth;
            return ret;
        },
        __wbg_new_02d162bc6cf02f60: function() {
            const ret = new Object();
            return ret;
        },
        __wbg_new_0_2722fcdb71a888a6: function() {
            const ret = new Date();
            return ret;
        },
        __wbg_new_310879b66b6e95e1: function() {
            const ret = new Array();
            return ret;
        },
        __wbg_new_439823ec67f1ec48: function() { return handleError(function (arg0, arg1) {
            const ret = new OffscreenCanvas(arg0 >>> 0, arg1 >>> 0);
            return ret;
        }, arguments); },
        __wbg_new_b1280f836646084c: function() { return handleError(function (arg0, arg1) {
            const ret = new WebSocket(getStringFromWasm0(arg0, arg1));
            return ret;
        }, arguments); },
        __wbg_new_from_slice_7a419f18ea6472bf: function(arg0, arg1) {
            const ret = new Float32Array(getArrayF32FromWasm0(arg0, arg1));
            return ret;
        },
        __wbg_of_d694dacacb7afa7f: function(arg0) {
            const ret = Array.of(arg0);
            return ret;
        },
        __wbg_offsetX_33e541f377c3ef38: function(arg0) {
            const ret = arg0.offsetX;
            return ret;
        },
        __wbg_offsetY_79c8223b1054870e: function(arg0) {
            const ret = arg0.offsetY;
            return ret;
        },
        __wbg_parentElement_6da9938b7d952c3c: function(arg0) {
            const ret = arg0.parentElement;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_pointerLockElement_f7f73e0720db5ba4: function(arg0) {
            const ret = arg0.pointerLockElement;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_polygonOffset_690c52c5bfca2a27: function(arg0, arg1, arg2) {
            arg0.polygonOffset(arg1, arg2);
        },
        __wbg_preventDefault_077a15ca7e97dc5a: function(arg0) {
            arg0.preventDefault();
        },
        __wbg_push_b77c476b01548d0a: function(arg0, arg1) {
            const ret = arg0.push(arg1);
            return ret;
        },
        __wbg_queueMicrotask_78d584b53af520f5: function(arg0) {
            const ret = arg0.queueMicrotask;
            return ret;
        },
        __wbg_queueMicrotask_b39ea83c7f01971a: function(arg0) {
            queueMicrotask(arg0);
        },
        __wbg_readBuffer_dc685ea6f3a7d5aa: function(arg0, arg1) {
            arg0.readBuffer(arg1 >>> 0);
        },
        __wbg_readPixels_a001bc59b9ea69e5: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8) {
            arg0.readPixels(arg1, arg2, arg3, arg4, arg5 >>> 0, arg6 >>> 0, arg7 === 0 ? undefined : getArrayU8FromWasm0(arg7, arg8));
        }, arguments); },
        __wbg_readyState_a1a00cc8898812ac: function(arg0) {
            const ret = arg0.readyState;
            return ret;
        },
        __wbg_removeAttribute_69b4f669d167f410: function() { return handleError(function (arg0, arg1, arg2) {
            arg0.removeAttribute(getStringFromWasm0(arg1, arg2));
        }, arguments); },
        __wbg_remove_e954ec2032f89c87: function(arg0) {
            arg0.remove();
        },
        __wbg_resolve_d17db9352f5a220e: function(arg0) {
            const ret = Promise.resolve(arg0);
            return ret;
        },
        __wbg_right_4234ac428a2d661c: function(arg0) {
            const ret = arg0.right;
            return ret;
        },
        __wbg_samplerParameterf_eb39264d0b3431ea: function(arg0, arg1, arg2, arg3) {
            arg0.samplerParameterf(arg1, arg2 >>> 0, arg3);
        },
        __wbg_samplerParameteri_7a90e6197a393b63: function(arg0, arg1, arg2, arg3) {
            arg0.samplerParameteri(arg1, arg2 >>> 0, arg3);
        },
        __wbg_scale_fc1a54fbad588f52: function() { return handleError(function (arg0, arg1, arg2) {
            arg0.scale(arg1, arg2);
        }, arguments); },
        __wbg_scrollHeight_9fb765c6a1d787d9: function(arg0) {
            const ret = arg0.scrollHeight;
            return ret;
        },
        __wbg_scrollLeft_04751bc428972599: function(arg0) {
            const ret = arg0.scrollLeft;
            return ret;
        },
        __wbg_scrollTop_9a72d5718621fde7: function(arg0) {
            const ret = arg0.scrollTop;
            return ret;
        },
        __wbg_send_e1d2f71ce4473d1e: function() { return handleError(function (arg0, arg1, arg2) {
            arg0.send(getStringFromWasm0(arg1, arg2));
        }, arguments); },
        __wbg_setAttribute_2e611c7b4151e535: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4) {
            arg0.setAttribute(getStringFromWasm0(arg1, arg2), getStringFromWasm0(arg3, arg4));
        }, arguments); },
        __wbg_setItem_caab843cd6845dbb: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4) {
            arg0.setItem(getStringFromWasm0(arg1, arg2), getStringFromWasm0(arg3, arg4));
        }, arguments); },
        __wbg_setProperty_ee784b2651f9ff8d: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4) {
            arg0.setProperty(getStringFromWasm0(arg1, arg2), getStringFromWasm0(arg3, arg4));
        }, arguments); },
        __wbg_set_718c83517b18990d: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4) {
            arg0[getStringFromWasm0(arg1, arg2)] = getStringFromWasm0(arg3, arg4);
        }, arguments); },
        __wbg_set_a0e911be3da02782: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = Reflect.set(arg0, arg1, arg2);
            return ret;
        }, arguments); },
        __wbg_set_alt_57ca52e99366b08e: function(arg0, arg1, arg2) {
            arg0.alt = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_checked_8aed6e848875c92d: function(arg0, arg1) {
            arg0.checked = arg1 !== 0;
        },
        __wbg_set_className_891d1e531a061f66: function(arg0, arg1, arg2) {
            arg0.className = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_draggable_4b941675ae166ab8: function(arg0, arg1) {
            arg0.draggable = arg1 !== 0;
        },
        __wbg_set_fillStyle_ac68c79af375566e: function(arg0, arg1, arg2) {
            arg0.fillStyle = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_font_fd5a0234c8e2e2eb: function(arg0, arg1, arg2) {
            arg0.font = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_globalCompositeOperation_884b1f7507e6f565: function() { return handleError(function (arg0, arg1, arg2) {
            arg0.globalCompositeOperation = getStringFromWasm0(arg1, arg2);
        }, arguments); },
        __wbg_set_height_bdd58e6b04e88cca: function(arg0, arg1) {
            arg0.height = arg1 >>> 0;
        },
        __wbg_set_hidden_dfe3317b7e220857: function(arg0, arg1) {
            arg0.hidden = arg1 !== 0;
        },
        __wbg_set_onclose_3121e15055418a37: function(arg0, arg1) {
            arg0.onclose = arg1;
        },
        __wbg_set_onload_19375fe3b5efc4f5: function(arg0, arg1) {
            arg0.onload = arg1;
        },
        __wbg_set_onmessage_9eb2cf76e70783ad: function(arg0, arg1) {
            arg0.onmessage = arg1;
        },
        __wbg_set_placeholder_a6e7fe14a6b96a34: function(arg0, arg1, arg2) {
            arg0.placeholder = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_scrollLeft_caf1432eea4f3239: function(arg0, arg1) {
            arg0.scrollLeft = arg1;
        },
        __wbg_set_scrollTop_3e985371212cf0f3: function(arg0, arg1) {
            arg0.scrollTop = arg1;
        },
        __wbg_set_src_d1764443f29632b8: function(arg0, arg1, arg2) {
            arg0.src = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_tabIndex_5f434f485ccd1a07: function(arg0, arg1) {
            arg0.tabIndex = arg1;
        },
        __wbg_set_textContent_9c5d65d703443b6d: function(arg0, arg1, arg2) {
            arg0.textContent = arg1 === 0 ? undefined : getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_type_4aa0902fbcc75add: function(arg0, arg1, arg2) {
            arg0.type = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_value_909e86868544a9f3: function(arg0, arg1, arg2) {
            arg0.value = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_width_25112eb6bf1148df: function(arg0, arg1) {
            arg0.width = arg1 >>> 0;
        },
        __wbg_shaderSource_eceb56c4b827824d: function(arg0, arg1, arg2, arg3) {
            arg0.shaderSource(arg1, getStringFromWasm0(arg2, arg3));
        },
        __wbg_solarImageUrl_8dfc9006a2eed90a: function() { return handleError(function (arg0, arg1) {
            const ret = solarImageUrl(getStringFromWasm0(arg0, arg1));
            return ret;
        }, arguments); },
        __wbg_static_accessor_GLOBAL_THIS_02344c9b09eb08a9: function() {
            const ret = typeof globalThis === 'undefined' ? null : globalThis;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_static_accessor_GLOBAL_ac6d4ac874d5cd54: function() {
            const ret = typeof global === 'undefined' ? null : global;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_static_accessor_SELF_9b2406c23aeb2023: function() {
            const ret = typeof self === 'undefined' ? null : self;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_static_accessor_WINDOW_b34d2126934e16ba: function() {
            const ret = typeof window === 'undefined' ? null : window;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_stopPropagation_fc3493778c4d8ac7: function(arg0) {
            arg0.stopPropagation();
        },
        __wbg_style_403ebe345c76f9f5: function(arg0) {
            const ret = arg0.style;
            return ret;
        },
        __wbg_tagName_7a06e2f76855c9f5: function(arg0, arg1) {
            const ret = arg1.tagName;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc_command_export, wasm.__wbindgen_realloc_command_export);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg_texImage2D_60fdb1271ceedd96: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9, arg10) {
            arg0.texImage2D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7 >>> 0, arg8 >>> 0, arg9 === 0 ? undefined : getArrayU8FromWasm0(arg9, arg10));
        }, arguments); },
        __wbg_texImage2D_c32967036972d3a3: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6) {
            arg0.texImage2D(arg1 >>> 0, arg2, arg3, arg4 >>> 0, arg5 >>> 0, arg6);
        }, arguments); },
        __wbg_texParameteri_c22838926a5dca2b: function(arg0, arg1, arg2, arg3) {
            arg0.texParameteri(arg1 >>> 0, arg2 >>> 0, arg3);
        },
        __wbg_texStorage2D_afb762382f8a4678: function(arg0, arg1, arg2, arg3, arg4, arg5) {
            arg0.texStorage2D(arg1 >>> 0, arg2, arg3 >>> 0, arg4, arg5);
        },
        __wbg_textContent_ac14c773b5616f3f: function(arg0, arg1) {
            const ret = arg1.textContent;
            var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_malloc_command_export, wasm.__wbindgen_realloc_command_export);
            var len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg_text_99930d92d5f1b540: function() { return handleError(function (arg0) {
            const ret = arg0.text();
            return ret;
        }, arguments); },
        __wbg_then_837494e384b37459: function(arg0, arg1) {
            const ret = arg0.then(arg1);
            return ret;
        },
        __wbg_then_bd927500e8905df2: function(arg0, arg1, arg2) {
            const ret = arg0.then(arg1, arg2);
            return ret;
        },
        __wbg_toggle_2d5806f414676c4a: function() { return handleError(function (arg0, arg1, arg2, arg3) {
            const ret = arg0.toggle(getStringFromWasm0(arg1, arg2), arg3 !== 0);
            return ret;
        }, arguments); },
        __wbg_top_e647a4c02850ada7: function(arg0) {
            const ret = arg0.top;
            return ret;
        },
        __wbg_uniform1f_709baed741125e5e: function(arg0, arg1, arg2) {
            arg0.uniform1f(arg1, arg2);
        },
        __wbg_uniform1fv_0f4ef61a14727a2d: function(arg0, arg1, arg2, arg3) {
            arg0.uniform1fv(arg1, getArrayF32FromWasm0(arg2, arg3));
        },
        __wbg_uniform1i_717096cfb8ca6bc1: function(arg0, arg1, arg2) {
            arg0.uniform1i(arg1, arg2);
        },
        __wbg_uniform2f_b04f3555fd2251e5: function(arg0, arg1, arg2, arg3) {
            arg0.uniform2f(arg1, arg2, arg3);
        },
        __wbg_uniform3f_501f926516b72519: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.uniform3f(arg1, arg2, arg3, arg4);
        },
        __wbg_uniform4f_7bc8db9ead983de4: function(arg0, arg1, arg2, arg3, arg4, arg5) {
            arg0.uniform4f(arg1, arg2, arg3, arg4, arg5);
        },
        __wbg_uniformMatrix4fv_d2b5005a92d27115: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.uniformMatrix4fv(arg1, arg2 !== 0, getArrayF32FromWasm0(arg3, arg4));
        },
        __wbg_useProgram_3cc1a6d58dac88b4: function(arg0, arg1) {
            arg0.useProgram(arg1);
        },
        __wbg_value_6cc2e59057b00287: function(arg0, arg1) {
            const ret = arg1.value;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc_command_export, wasm.__wbindgen_realloc_command_export);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg_value_bfbe8f2b86b7ad71: function(arg0, arg1) {
            const ret = arg1.value;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc_command_export, wasm.__wbindgen_realloc_command_export);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg_vertexAttribPointer_85566c79cb366300: function(arg0, arg1, arg2, arg3, arg4, arg5, arg6) {
            arg0.vertexAttribPointer(arg1 >>> 0, arg2, arg3 >>> 0, arg4 !== 0, arg5, arg6);
        },
        __wbg_viewport_3c149d0c6435f0ed: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.viewport(arg1, arg2, arg3, arg4);
        },
        __wbg_width_5894e5b3a7673417: function(arg0) {
            const ret = arg0.width;
            return ret;
        },
        __wbg_width_e987166926c3367c: function(arg0) {
            const ret = arg0.width;
            return ret;
        },
        __wbindgen_cast_0000000000000001: function(arg0, arg1) {
            // Cast intrinsic for `Closure(Closure { owned: true, function: Function { arguments: [Externref], shim_idx: 1144, ret: Result(Unit), inner_ret: Some(Result(Unit)) }, mutable: true }) -> Externref`.
            const ret = makeMutClosure(arg0, arg1, wasm_bindgen_c5f06a7440465467___convert__closures_____invoke___wasm_bindgen_c5f06a7440465467___JsValue__core_d941ad8722425d2e___result__Result_____wasm_bindgen_c5f06a7440465467___JsError___true_);
            return ret;
        },
        __wbindgen_cast_0000000000000002: function(arg0, arg1) {
            // Cast intrinsic for `Closure(Closure { owned: true, function: Function { arguments: [NamedExternref("CloseEvent")], shim_idx: 22, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
            const ret = makeMutClosure(arg0, arg1, wasm_bindgen_c5f06a7440465467___convert__closures_____invoke___web_sys_89cab73c7b663fe4___features__gen_CloseEvent__CloseEvent______true_);
            return ret;
        },
        __wbindgen_cast_0000000000000003: function(arg0, arg1) {
            // Cast intrinsic for `Closure(Closure { owned: true, function: Function { arguments: [NamedExternref("Event")], shim_idx: 22, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
            const ret = makeMutClosure(arg0, arg1, wasm_bindgen_c5f06a7440465467___convert__closures_____invoke___web_sys_89cab73c7b663fe4___features__gen_CloseEvent__CloseEvent______true__2);
            return ret;
        },
        __wbindgen_cast_0000000000000004: function(arg0, arg1) {
            // Cast intrinsic for `Closure(Closure { owned: true, function: Function { arguments: [NamedExternref("KeyboardEvent")], shim_idx: 22, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
            const ret = makeMutClosure(arg0, arg1, wasm_bindgen_c5f06a7440465467___convert__closures_____invoke___web_sys_89cab73c7b663fe4___features__gen_CloseEvent__CloseEvent______true__3);
            return ret;
        },
        __wbindgen_cast_0000000000000005: function(arg0, arg1) {
            // Cast intrinsic for `Closure(Closure { owned: true, function: Function { arguments: [NamedExternref("MessageEvent")], shim_idx: 22, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
            const ret = makeMutClosure(arg0, arg1, wasm_bindgen_c5f06a7440465467___convert__closures_____invoke___web_sys_89cab73c7b663fe4___features__gen_CloseEvent__CloseEvent______true__4);
            return ret;
        },
        __wbindgen_cast_0000000000000006: function(arg0, arg1) {
            // Cast intrinsic for `Closure(Closure { owned: true, function: Function { arguments: [NamedExternref("MouseEvent")], shim_idx: 22, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
            const ret = makeMutClosure(arg0, arg1, wasm_bindgen_c5f06a7440465467___convert__closures_____invoke___web_sys_89cab73c7b663fe4___features__gen_CloseEvent__CloseEvent______true__5);
            return ret;
        },
        __wbindgen_cast_0000000000000007: function(arg0, arg1) {
            // Cast intrinsic for `Closure(Closure { owned: true, function: Function { arguments: [NamedExternref("WheelEvent")], shim_idx: 22, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
            const ret = makeMutClosure(arg0, arg1, wasm_bindgen_c5f06a7440465467___convert__closures_____invoke___web_sys_89cab73c7b663fe4___features__gen_CloseEvent__CloseEvent______true__6);
            return ret;
        },
        __wbindgen_cast_0000000000000008: function(arg0) {
            // Cast intrinsic for `F64 -> Externref`.
            const ret = arg0;
            return ret;
        },
        __wbindgen_cast_0000000000000009: function(arg0, arg1) {
            // Cast intrinsic for `Ref(Slice(F32)) -> NamedExternref("Float32Array")`.
            const ret = getArrayF32FromWasm0(arg0, arg1);
            return ret;
        },
        __wbindgen_cast_000000000000000a: function(arg0, arg1) {
            // Cast intrinsic for `Ref(String) -> Externref`.
            const ret = getStringFromWasm0(arg0, arg1);
            return ret;
        },
        __wbindgen_init_externref_table: function() {
            const table = wasm.__wbindgen_externrefs;
            const offset = table.grow(4);
            table.set(0, undefined);
            table.set(offset + 0, undefined);
            table.set(offset + 1, null);
            table.set(offset + 2, true);
            table.set(offset + 3, false);
        },
    };
    return {
        __proto__: null,
        "./swirl_client_bg.js": import0,
        "wasi_snapshot_preview1": swirlWasi(),
    };
}

function wasm_bindgen_c5f06a7440465467___convert__closures_____invoke___web_sys_89cab73c7b663fe4___features__gen_CloseEvent__CloseEvent______true_(arg0, arg1, arg2) {
    wasm.wasm_bindgen_c5f06a7440465467___convert__closures_____invoke___web_sys_89cab73c7b663fe4___features__gen_CloseEvent__CloseEvent______true_(arg0, arg1, arg2);
}

function wasm_bindgen_c5f06a7440465467___convert__closures_____invoke___web_sys_89cab73c7b663fe4___features__gen_CloseEvent__CloseEvent______true__2(arg0, arg1, arg2) {
    wasm.wasm_bindgen_c5f06a7440465467___convert__closures_____invoke___web_sys_89cab73c7b663fe4___features__gen_CloseEvent__CloseEvent______true__2(arg0, arg1, arg2);
}

function wasm_bindgen_c5f06a7440465467___convert__closures_____invoke___web_sys_89cab73c7b663fe4___features__gen_CloseEvent__CloseEvent______true__3(arg0, arg1, arg2) {
    wasm.wasm_bindgen_c5f06a7440465467___convert__closures_____invoke___web_sys_89cab73c7b663fe4___features__gen_CloseEvent__CloseEvent______true__3(arg0, arg1, arg2);
}

function wasm_bindgen_c5f06a7440465467___convert__closures_____invoke___web_sys_89cab73c7b663fe4___features__gen_CloseEvent__CloseEvent______true__4(arg0, arg1, arg2) {
    wasm.wasm_bindgen_c5f06a7440465467___convert__closures_____invoke___web_sys_89cab73c7b663fe4___features__gen_CloseEvent__CloseEvent______true__4(arg0, arg1, arg2);
}

function wasm_bindgen_c5f06a7440465467___convert__closures_____invoke___web_sys_89cab73c7b663fe4___features__gen_CloseEvent__CloseEvent______true__5(arg0, arg1, arg2) {
    wasm.wasm_bindgen_c5f06a7440465467___convert__closures_____invoke___web_sys_89cab73c7b663fe4___features__gen_CloseEvent__CloseEvent______true__5(arg0, arg1, arg2);
}

function wasm_bindgen_c5f06a7440465467___convert__closures_____invoke___web_sys_89cab73c7b663fe4___features__gen_CloseEvent__CloseEvent______true__6(arg0, arg1, arg2) {
    wasm.wasm_bindgen_c5f06a7440465467___convert__closures_____invoke___web_sys_89cab73c7b663fe4___features__gen_CloseEvent__CloseEvent______true__6(arg0, arg1, arg2);
}

function wasm_bindgen_c5f06a7440465467___convert__closures_____invoke___wasm_bindgen_c5f06a7440465467___JsValue__core_d941ad8722425d2e___result__Result_____wasm_bindgen_c5f06a7440465467___JsError___true_(arg0, arg1, arg2) {
    const ret = wasm.wasm_bindgen_c5f06a7440465467___convert__closures_____invoke___wasm_bindgen_c5f06a7440465467___JsValue__core_d941ad8722425d2e___result__Result_____wasm_bindgen_c5f06a7440465467___JsError___true_(arg0, arg1, arg2);
    if (ret[1]) {
        throw takeFromExternrefTable0(ret[0]);
    }
}

const BootFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_boot_free(ptr, 1));
const ClientFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_client_free(ptr, 1));
const DevConsoleFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_devconsole_free(ptr, 1));
const LuaVmFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_luavm_free(ptr, 1));
const SceneFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_scene_free(ptr, 1));
const WebMeasureFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_webmeasure_free(ptr, 1));

function addToExternrefTable0(obj) {
    const idx = wasm.__externref_table_alloc_command_export();
    wasm.__wbindgen_externrefs.set(idx, obj);
    return idx;
}

const CLOSURE_DTORS = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(state => wasm.__wbindgen_destroy_closure_command_export(state.a, state.b));

function debugString(val) {
    // primitive types
    const type = typeof val;
    if (type == 'number' || type == 'boolean' || val == null) {
        return  `${val}`;
    }
    if (type == 'string') {
        return `"${val}"`;
    }
    if (type == 'symbol') {
        const description = val.description;
        if (description == null) {
            return 'Symbol';
        } else {
            return `Symbol(${description})`;
        }
    }
    if (type == 'function') {
        const name = val.name;
        if (typeof name == 'string' && name.length > 0) {
            return `Function(${name})`;
        } else {
            return 'Function';
        }
    }
    // objects
    if (Array.isArray(val)) {
        const length = val.length;
        let debug = '[';
        if (length > 0) {
            debug += debugString(val[0]);
        }
        for(let i = 1; i < length; i++) {
            debug += ', ' + debugString(val[i]);
        }
        debug += ']';
        return debug;
    }
    // Test for built-in
    const builtInMatches = /\[object ([^\]]+)\]/.exec(toString.call(val));
    let className;
    if (builtInMatches && builtInMatches.length > 1) {
        className = builtInMatches[1];
    } else {
        // Failed to match the standard '[object ClassName]'
        return toString.call(val);
    }
    if (className == 'Object') {
        // we're a user defined class or Object
        // JSON.stringify avoids problems with cycles, and is generally much
        // easier than looping through ownProperties of `val`.
        try {
            return 'Object(' + JSON.stringify(val) + ')';
        } catch (_) {
            return 'Object';
        }
    }
    // errors
    if (val instanceof Error) {
        return `${val.name}: ${val.message}\n${val.stack}`;
    }
    // TODO we could test for more things here, like `Set`s and `Map`s.
    return className;
}

function getArrayF32FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getFloat32ArrayMemory0().subarray(ptr / 4, ptr / 4 + len);
}

function getArrayF64FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getFloat64ArrayMemory0().subarray(ptr / 8, ptr / 8 + len);
}

function getArrayU32FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getUint32ArrayMemory0().subarray(ptr / 4, ptr / 4 + len);
}

function getArrayU8FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getUint8ArrayMemory0().subarray(ptr / 1, ptr / 1 + len);
}

let cachedDataViewMemory0 = null;
function getDataViewMemory0() {
    if (cachedDataViewMemory0 === null || cachedDataViewMemory0.buffer.detached === true || (cachedDataViewMemory0.buffer.detached === undefined && cachedDataViewMemory0.buffer !== wasm.memory.buffer)) {
        cachedDataViewMemory0 = new DataView(wasm.memory.buffer);
    }
    return cachedDataViewMemory0;
}

let cachedFloat32ArrayMemory0 = null;
function getFloat32ArrayMemory0() {
    if (cachedFloat32ArrayMemory0 === null || cachedFloat32ArrayMemory0.byteLength === 0) {
        cachedFloat32ArrayMemory0 = new Float32Array(wasm.memory.buffer);
    }
    return cachedFloat32ArrayMemory0;
}

let cachedFloat64ArrayMemory0 = null;
function getFloat64ArrayMemory0() {
    if (cachedFloat64ArrayMemory0 === null || cachedFloat64ArrayMemory0.byteLength === 0) {
        cachedFloat64ArrayMemory0 = new Float64Array(wasm.memory.buffer);
    }
    return cachedFloat64ArrayMemory0;
}

function getStringFromWasm0(ptr, len) {
    return decodeText(ptr >>> 0, len);
}

let cachedUint32ArrayMemory0 = null;
function getUint32ArrayMemory0() {
    if (cachedUint32ArrayMemory0 === null || cachedUint32ArrayMemory0.byteLength === 0) {
        cachedUint32ArrayMemory0 = new Uint32Array(wasm.memory.buffer);
    }
    return cachedUint32ArrayMemory0;
}

let cachedUint8ArrayMemory0 = null;
function getUint8ArrayMemory0() {
    if (cachedUint8ArrayMemory0 === null || cachedUint8ArrayMemory0.byteLength === 0) {
        cachedUint8ArrayMemory0 = new Uint8Array(wasm.memory.buffer);
    }
    return cachedUint8ArrayMemory0;
}

function handleError(f, args) {
    try {
        return f.apply(this, args);
    } catch (e) {
        const idx = addToExternrefTable0(e);
        wasm.__wbindgen_exn_store_command_export(idx);
    }
}

function isLikeNone(x) {
    return x === undefined || x === null;
}

function makeMutClosure(arg0, arg1, f) {
    const state = { a: arg0, b: arg1, cnt: 1 };
    const real = (...args) => {

        // First up with a closure we increment the internal reference
        // count. This ensures that the Rust closure environment won't
        // be deallocated while we're invoking it.
        state.cnt++;
        const a = state.a;
        state.a = 0;
        try {
            return f(a, state.b, ...args);
        } finally {
            state.a = a;
            real._wbg_cb_unref();
        }
    };
    real._wbg_cb_unref = () => {
        if (--state.cnt === 0) {
            wasm.__wbindgen_destroy_closure_command_export(state.a, state.b);
            state.a = 0;
            CLOSURE_DTORS.unregister(state);
        }
    };
    CLOSURE_DTORS.register(real, state, state);
    return real;
}

function passArray8ToWasm0(arg, malloc) {
    const ptr = malloc(arg.length * 1, 1) >>> 0;
    getUint8ArrayMemory0().set(arg, ptr / 1);
    WASM_VECTOR_LEN = arg.length;
    return ptr;
}

function passArrayF32ToWasm0(arg, malloc) {
    const ptr = malloc(arg.length * 4, 4) >>> 0;
    getFloat32ArrayMemory0().set(arg, ptr / 4);
    WASM_VECTOR_LEN = arg.length;
    return ptr;
}

function passArrayF64ToWasm0(arg, malloc) {
    const ptr = malloc(arg.length * 8, 8) >>> 0;
    getFloat64ArrayMemory0().set(arg, ptr / 8);
    WASM_VECTOR_LEN = arg.length;
    return ptr;
}

function passStringToWasm0(arg, malloc, realloc) {
    if (realloc === undefined) {
        const buf = cachedTextEncoder.encode(arg);
        const ptr = malloc(buf.length, 1) >>> 0;
        getUint8ArrayMemory0().subarray(ptr, ptr + buf.length).set(buf);
        WASM_VECTOR_LEN = buf.length;
        return ptr;
    }

    let len = arg.length;
    let ptr = malloc(len, 1) >>> 0;

    const mem = getUint8ArrayMemory0();

    let offset = 0;

    for (; offset < len; offset++) {
        const code = arg.charCodeAt(offset);
        if (code > 0x7F) break;
        mem[ptr + offset] = code;
    }
    if (offset !== len) {
        if (offset !== 0) {
            arg = arg.slice(offset);
        }
        ptr = realloc(ptr, len, len = offset + arg.length * 3, 1) >>> 0;
        const view = getUint8ArrayMemory0().subarray(ptr + offset, ptr + len);
        const ret = cachedTextEncoder.encodeInto(arg, view);

        offset += ret.written;
        ptr = realloc(ptr, len, offset, 1) >>> 0;
    }

    WASM_VECTOR_LEN = offset;
    return ptr;
}

function takeFromExternrefTable0(idx) {
    const value = wasm.__wbindgen_externrefs.get(idx);
    wasm.__externref_table_dealloc_command_export(idx);
    return value;
}

let cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
cachedTextDecoder.decode();
const MAX_SAFARI_DECODE_BYTES = 2146435072;
let numBytesDecoded = 0;
function decodeText(ptr, len) {
    numBytesDecoded += len;
    if (numBytesDecoded >= MAX_SAFARI_DECODE_BYTES) {
        cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
        cachedTextDecoder.decode();
        numBytesDecoded = len;
    }
    return cachedTextDecoder.decode(getUint8ArrayMemory0().subarray(ptr, ptr + len));
}

const cachedTextEncoder = new TextEncoder();

if (!('encodeInto' in cachedTextEncoder)) {
    cachedTextEncoder.encodeInto = function (arg, view) {
        const buf = cachedTextEncoder.encode(arg);
        view.set(buf);
        return {
            read: arg.length,
            written: buf.length
        };
    };
}

let WASM_VECTOR_LEN = 0;

let wasmModule, wasmInstance, wasm;
function __wbg_finalize_init(instance, module) {
    wasmInstance = instance;
    wasm = instance.exports;
    wasmModule = module;
    cachedDataViewMemory0 = null;
    cachedFloat32ArrayMemory0 = null;
    cachedFloat64ArrayMemory0 = null;
    cachedUint32ArrayMemory0 = null;
    cachedUint8ArrayMemory0 = null;
    wasm.__wbindgen_start();
    return wasm;
}

async function __wbg_load(module, imports) {
    if (typeof Response === 'function' && module instanceof Response) {
        if (typeof WebAssembly.instantiateStreaming === 'function') {
            try {
                return await WebAssembly.instantiateStreaming(module, imports);
            } catch (e) {
                const validResponse = module.ok && expectedResponseType(module.type);

                if (validResponse && module.headers.get('Content-Type') !== 'application/wasm') {
                    console.warn("`WebAssembly.instantiateStreaming` failed because your server does not serve Wasm with `application/wasm` MIME type. Falling back to `WebAssembly.instantiate` which is slower. Original error:\n", e);

                } else { throw e; }
            }
        }

        const bytes = await module.arrayBuffer();
        return await WebAssembly.instantiate(bytes, imports);
    } else {
        const instance = await WebAssembly.instantiate(module, imports);

        if (instance instanceof WebAssembly.Instance) {
            return { instance, module };
        } else {
            return instance;
        }
    }

    function expectedResponseType(type) {
        switch (type) {
            case 'basic': case 'cors': case 'default': return true;
        }
        return false;
    }
}

function initSync(module) {
    if (wasm !== undefined) return wasm;


    if (module !== undefined) {
        if (Object.getPrototypeOf(module) === Object.prototype) {
            ({module} = module)
        } else {
            console.warn('using deprecated parameters for `initSync()`; pass a single object instead')
        }
    }

    const imports = __wbg_get_imports();
    if (!(module instanceof WebAssembly.Module)) {
        module = new WebAssembly.Module(module);
    }
    const instance = new WebAssembly.Instance(module, imports);
    return __wbg_finalize_init(instance, module);
}

async function __wbg_init(module_or_path) {
    if (wasm !== undefined) return wasm;


    if (module_or_path !== undefined) {
        if (Object.getPrototypeOf(module_or_path) === Object.prototype) {
            ({module_or_path} = module_or_path)
        } else {
            console.warn('using deprecated parameters for the initialization function; pass a single object instead')
        }
    }

    if (module_or_path === undefined) {
        module_or_path = new URL('swirl_client_bg.wasm', import.meta.url);
    }
    const imports = __wbg_get_imports();

    if (typeof module_or_path === 'string' || (typeof Request === 'function' && module_or_path instanceof Request) || (typeof URL === 'function' && module_or_path instanceof URL)) {
        module_or_path = fetch(module_or_path);
    }

    const { instance, module } = await __wbg_load(await module_or_path, imports);

    return __wbg_finalize_init(instance, module);
}

export { initSync, __wbg_init as default };
