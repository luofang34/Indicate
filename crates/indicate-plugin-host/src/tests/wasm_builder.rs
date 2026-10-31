//! Pure-Rust Wasm binary module encoder for `.indpack` sandbox tests.
#![allow(clippy::expect_used)]

pub(super) fn push_leb_u32(out: &mut Vec<u8>, mut val: u32) {
    loop {
        let mut byte = (val & 0x7F) as u8;
        val >>= 7;
        if val != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if val == 0 {
            break;
        }
    }
}

pub(super) fn push_leb_i32(out: &mut Vec<u8>, mut val: i32) {
    loop {
        let byte = (val & 0x7F) as u8;
        val >>= 7;
        let done = (val == 0 && (byte & 0x40) == 0) || (val == -1 && (byte & 0x40) != 0);
        if done {
            out.push(byte);
            break;
        }
        out.push(byte | 0x80);
    }
}

fn push_section(wasm: &mut Vec<u8>, id: u8, payload: &[u8]) {
    wasm.push(id);
    push_leb_u32(wasm, payload.len() as u32);
    wasm.extend_from_slice(payload);
}

fn push_export(sec: &mut Vec<u8>, name: &str, kind: u8, idx: u32) {
    push_leb_u32(sec, name.len() as u32);
    sec.extend_from_slice(name.as_bytes());
    sec.push(kind);
    push_leb_u32(sec, idx);
}

/// Builds a valid Wasm module exporting `memory` (`1..=max_pages`),
/// `ind_abi_version() -> 1`, `ind_scratch_ptr() -> 4096`,
/// `ind_scratch_cap() -> 61440`, and `ind_draw(...) -> i32` with `draw_body`
/// (with `local_i32_count` extra `i32` locals) plus static `data_seg` at offset `0`.
pub(super) fn build_plugin_wasm(
    max_pages: u32,
    local_i32_count: u32,
    draw_body: &[u8],
    data_seg: &[u8],
) -> Vec<u8> {
    let mut wasm = Vec::from(*b"\0asm\x01\0\0\0");

    // Type section (id=1): type 0 = () -> i32; type 1 = (7 x i32) -> i32
    let mut types = Vec::new();
    push_leb_u32(&mut types, 2);
    types.extend_from_slice(&[0x60, 0x00, 0x01, 0x7F]);
    types.extend_from_slice(&[
        0x60, 0x07, 0x7F, 0x7F, 0x7F, 0x7F, 0x7F, 0x7F, 0x7F, 0x01, 0x7F,
    ]);
    push_section(&mut wasm, 1, &types);

    // Function section (id=3): funcs 0,1,2 -> type 0; func 3 -> type 1
    let funcs = [4u8, 0, 0, 0, 1];
    push_section(&mut wasm, 3, &funcs);

    // Memory section (id=5): 1 memory with limits (flags=1 has_max, min=1, max=max_pages)
    let mut mem = vec![1u8, 0x01];
    push_leb_u32(&mut mem, 1);
    push_leb_u32(&mut mem, max_pages);
    push_section(&mut wasm, 5, &mem);

    // Export section (id=7): memory + 4 ABI functions
    let mut exports = Vec::new();
    push_leb_u32(&mut exports, 5);
    push_export(&mut exports, "memory", 0x02, 0);
    push_export(&mut exports, "ind_abi_version", 0x00, 0);
    push_export(&mut exports, "ind_scratch_ptr", 0x00, 1);
    push_export(&mut exports, "ind_scratch_cap", 0x00, 2);
    push_export(&mut exports, "ind_draw", 0x00, 3);
    push_section(&mut wasm, 7, &exports);

    // Code section (id=10)
    let mut code = Vec::new();
    push_leb_u32(&mut code, 4);
    for val in [1i32, 4096i32, 61440i32] {
        let mut body = vec![0u8, 0x41];
        push_leb_i32(&mut body, val);
        body.push(0x0B);
        push_leb_u32(&mut code, body.len() as u32);
        code.extend_from_slice(&body);
    }
    let mut fn3 = Vec::new();
    if local_i32_count == 0 {
        fn3.push(0);
    } else {
        fn3.push(1);
        push_leb_u32(&mut fn3, local_i32_count);
        fn3.push(0x7F);
    }
    fn3.extend_from_slice(draw_body);
    fn3.push(0x0B);
    push_leb_u32(&mut code, fn3.len() as u32);
    code.extend_from_slice(&fn3);
    push_section(&mut wasm, 10, &code);

    // Data section (id=11) at offset 0
    if !data_seg.is_empty() {
        let mut data = Vec::new();
        push_leb_u32(&mut data, 1);
        data.extend_from_slice(&[0x00, 0x41, 0x00, 0x0B]);
        push_leb_u32(&mut data, data_seg.len() as u32);
        data.extend_from_slice(data_seg);
        push_section(&mut wasm, 11, &data);
    }
    wasm
}

/// Emits Wasm instructions copying `len` bytes from static offset `src` to `4096 + local.get(5)` and returning `len`.
pub(super) fn emit_static_copy_body(src: i32, len: usize) -> Vec<u8> {
    let mut b = Vec::new();
    for i in 0..len {
        // dst = 4096 + out_offset (param 5) + i
        b.push(0x41);
        push_leb_i32(&mut b, 4096 + i as i32);
        b.extend_from_slice(&[0x20, 0x05, 0x6A]);
        // val = i32.load8_u(src + i)
        b.push(0x41);
        push_leb_i32(&mut b, src + i as i32);
        b.extend_from_slice(&[0x2D, 0x00, 0x00]);
        // i32.store8
        b.extend_from_slice(&[0x3A, 0x00, 0x00]);
    }
    b.push(0x41);
    push_leb_i32(&mut b, len as i32);
    b
}
