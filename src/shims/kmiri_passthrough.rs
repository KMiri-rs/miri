use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use rustc_abi::{Align, CanonAbi, Size};
use rustc_middle::ty::Ty;
use rustc_span::Symbol;
use rustc_target::callconv::FnAbi;

use crate::*;

static PT_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
static PT_ALLOCS: OnceLock<Mutex<HashMap<u64, (u64, u64)>>> = OnceLock::new();
static PT_REQ_SEQ: AtomicU64 = AtomicU64::new(1);

impl<'tcx> EvalContextExt<'tcx> for crate::MiriInterpCx<'tcx> {}
pub trait EvalContextExt<'tcx>: crate::MiriInterpCxExt<'tcx> {
    fn emulate_bitmap_passthrough(
        &mut self,
        link_name: Symbol,
        abi: &FnAbi<'tcx, Ty<'tcx>>,
        args: &[OpTy<'tcx>],
        dest: &MPlaceTy<'tcx>,
    ) -> InterpResult<'tcx, crate::shims::EmulateItemResult> {
        let this = self.eval_context_mut();

        if passthrough_endpoint().is_none() {
            return interp_ok(crate::shims::EmulateItemResult::NotSupported);
        }

        if link_name.as_str().contains("bitmap") || link_name.as_str().contains("find_next") {
            eprintln!("[kmiri-passthrough] symbol={}", link_name.as_str());
        }

        match link_name.as_str() {
            "bitmap_zalloc" => {
                let [nbits, flags] = this.check_shim_sig_lenient(abi, CanonAbi::C, link_name, args)?;
                let nbits = this.read_scalar(nbits)?.to_u32()?;
                let flags = u64::from(this.read_scalar(flags)?.to_u32()?);
                let _ = send_request(&format!("bitmap_zalloc {nbits} {flags}"));

                let bits_per_long = this.pointer_size().bytes() * 8;
                let words = if nbits == 0 {
                    0
                } else {
                    u64::from(nbits).div_ceil(bits_per_long)
                };
                let size_bytes = words * this.pointer_size().bytes();
                let align = this.pointer_size().bytes();

                let alloc_bytes = size_bytes.max(1);
                let ptr = this.allocate_ptr(
                    Size::from_bytes(alloc_bytes),
                    Align::from_bytes(align).unwrap(),
                    MiriMemoryKind::C.into(),
                    rustc_middle::mir::interpret::AllocInit::Zero,
                )?;
                let key = crate::Pointer::from(ptr).addr().bytes();
                PT_ALLOCS
                    .get_or_init(|| Mutex::new(HashMap::new()))
                    .lock()
                    .expect("passthrough alloc map poisoned")
                    .insert(key, (alloc_bytes, align));

                this.write_pointer(ptr, dest)?;
                return interp_ok(crate::shims::EmulateItemResult::NeedsReturn);
            }
            "bitmap_free" => {
                let [ptr] = this.check_shim_sig_lenient(abi, CanonAbi::C, link_name, args)?;
                let ptr = this.read_pointer(ptr)?;
                let _ = send_request("bitmap_free");

                if ptr.addr().bytes() != 0 {
                    let key = ptr.addr().bytes();
                    if let Some((size_bytes, align)) = PT_ALLOCS
                        .get_or_init(|| Mutex::new(HashMap::new()))
                        .lock()
                        .expect("passthrough alloc map poisoned")
                        .remove(&key)
                    {
                        this.deallocate_ptr(
                            ptr,
                            Some((Size::from_bytes(size_bytes), Align::from_bytes(align).unwrap())),
                            MiriMemoryKind::C.into(),
                        )?;
                    }
                }
                return interp_ok(crate::shims::EmulateItemResult::NeedsReturn);
            }
            "rust_helper___set_bit" | "rust_helper_set_bit" | "rust_helper___clear_bit"
            | "rust_helper_clear_bit" => {
                let [index, ptr] = this.check_shim_sig_lenient(abi, CanonAbi::C, link_name, args)?;
                let index = this.read_target_usize(index)?;
                let ptr = this.read_pointer(ptr)?;

                let (alloc_id, _, _) = this.ptr_get_alloc_id(ptr, 0)?;
                let alloc_size = this.get_alloc_info(alloc_id).size.bytes();
                let nbits = alloc_size * 8;
                let bytes = this.read_bytes_ptr_strip_provenance(ptr, Size::from_bytes(alloc_size))?;
                let hex = hex_encode(bytes);
                let op = match link_name.as_str() {
                    "rust_helper___set_bit" => "set_bit",
                    "rust_helper_set_bit" => "set_bit_atomic",
                    "rust_helper___clear_bit" => "clear_bit",
                    "rust_helper_clear_bit" => "clear_bit_atomic",
                    _ => unreachable!(),
                };
                let resp = send_request(&format!("{op} {nbits} {index} {hex}"))
                    .map_err(|e| err_unsup_format!("kmiri passthrough failed: {e}"))?;
                let (_ret, hex_out) = parse_ok_ret_and_hex(&resp)
                    .map_err(|e| err_unsup_format!("kmiri passthrough parse failed: {e}"))?;
                let new_bytes = hex_decode(&hex_out)
                    .map_err(|e| err_unsup_format!("kmiri passthrough hex failed: {e}"))?;
                this.write_bytes_ptr(ptr, new_bytes.into_iter())?;
                return interp_ok(crate::shims::EmulateItemResult::NeedsReturn);
            }
            "_find_next_bit" | "_find_next_zero_bit" => {
                let [ptr, nbits, start] =
                    this.check_shim_sig_lenient(abi, CanonAbi::C, link_name, args)?;
                let ptr = this.read_pointer(ptr)?;
                let nbits = this.read_target_usize(nbits)?;
                let start = this.read_target_usize(start)?;
                let nbytes = nbits.div_ceil(8);
                let bytes = this.read_bytes_ptr_strip_provenance(ptr, Size::from_bytes(nbytes))?;
                let hex = hex_encode(bytes);
                let op = if link_name.as_str() == "_find_next_bit" {
                    "find_next_bit"
                } else {
                    "find_next_zero_bit"
                };
                let resp = send_request(&format!("{op} {nbits} {start} {hex}"))
                    .map_err(|e| err_unsup_format!("kmiri passthrough failed: {e}"))?;
                let ret = parse_ok_ret(&resp)
                    .map_err(|e| err_unsup_format!("kmiri passthrough parse failed: {e}"))?;
                this.write_scalar(Scalar::from_target_usize(ret, this), dest)?;
                return interp_ok(crate::shims::EmulateItemResult::NeedsReturn);
            }
            "_find_last_bit" => {
                let [ptr, nbits] = this.check_shim_sig_lenient(abi, CanonAbi::C, link_name, args)?;
                let ptr = this.read_pointer(ptr)?;
                let nbits = this.read_target_usize(nbits)?;
                let nbytes = nbits.div_ceil(8);
                let bytes = this.read_bytes_ptr_strip_provenance(ptr, Size::from_bytes(nbytes))?;
                let hex = hex_encode(bytes);
                let resp = send_request(&format!("find_last_bit {nbits} {hex}"))
                    .map_err(|e| err_unsup_format!("kmiri passthrough failed: {e}"))?;
                let ret = parse_ok_ret(&resp)
                    .map_err(|e| err_unsup_format!("kmiri passthrough parse failed: {e}"))?;
                this.write_scalar(Scalar::from_target_usize(ret, this), dest)?;
                return interp_ok(crate::shims::EmulateItemResult::NeedsReturn);
            }
            "rust_helper_bitmap_copy_and_extend" => {
                let [dst, src, count, size] =
                    this.check_shim_sig_lenient(abi, CanonAbi::C, link_name, args)?;
                let dst_ptr = this.read_pointer(dst)?;
                let src_ptr = this.read_pointer(src)?;
                let count = this.read_scalar(count)?.to_u32()?;
                let size = this.read_scalar(size)?.to_u32()?;

                let src_bytes_len = u64::from(count).div_ceil(8);
                let dst_bytes_len = u64::from(size).div_ceil(8);
                let src_bytes =
                    this.read_bytes_ptr_strip_provenance(src_ptr, Size::from_bytes(src_bytes_len))?;
                let dst_bytes =
                    this.read_bytes_ptr_strip_provenance(dst_ptr, Size::from_bytes(dst_bytes_len))?;

                let resp = send_request(&format!(
                    "bitmap_copy_and_extend {} {} {} {}",
                    count,
                    size,
                    hex_encode(src_bytes),
                    hex_encode(dst_bytes)
                ))
                .map_err(|e| err_unsup_format!("kmiri passthrough failed: {e}"))?;
                let (_ret, hex_out) = parse_ok_ret_and_hex(&resp)
                    .map_err(|e| err_unsup_format!("kmiri passthrough parse failed: {e}"))?;
                let out = hex_decode(&hex_out)
                    .map_err(|e| err_unsup_format!("kmiri passthrough hex failed: {e}"))?;
                this.write_bytes_ptr(dst_ptr, out.into_iter())?;
                return interp_ok(crate::shims::EmulateItemResult::NeedsReturn);
            }
            _ => {}
        }

        interp_ok(crate::shims::EmulateItemResult::NotSupported)
    }
}

fn passthrough_endpoint() -> Option<PassthroughEndpoint> {
    if let Some(proc_path) = std::env::var_os("KMIRI_PASSTHROUGH_PROC") {
        return Some(PassthroughEndpoint::Proc(PathBuf::from(proc_path)));
    }
    std::env::var_os("KMIRI_PASSTHROUGH_DIR").map(|p| PassthroughEndpoint::Dir(PathBuf::from(p)))
}

fn send_request(req: &str) -> io::Result<String> {
    let endpoint = passthrough_endpoint().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "KMIRI_PASSTHROUGH_PROC or KMIRI_PASSTHROUGH_DIR missing",
        )
    })?;
    let lock = PT_LOCK.get_or_init(|| Mutex::new(()));
    let _guard = lock.lock().expect("passthrough lock poisoned");

    match endpoint {
        PassthroughEndpoint::Proc(path) => send_request_via_proc(&path, req),
        PassthroughEndpoint::Dir(root) => send_request_via_dir(&root, req),
    }
}

fn send_request_via_proc(path: &Path, req: &str) -> io::Result<String> {
    fs::write(path, req.as_bytes())?;
    let resp = fs::read_to_string(path)?;
    let trimmed = resp.trim().to_string();
    if trimmed.is_empty() {
        return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "empty proc response"));
    }
    Ok(trimmed)
}

fn send_request_via_dir(root: &Path, req: &str) -> io::Result<String> {
    fs::create_dir_all(root)?;

    let seq = PT_REQ_SEQ.fetch_add(1, Ordering::Relaxed);
    let req_id = format!("{}-{}", std::process::id(), seq);
    let req_path = root.join(format!("req.{req_id}"));
    let resp_path = root.join(format!("resp.{req_id}"));

    fs::write(&req_path, req.as_bytes())?;

    let start = Instant::now();
    loop {
        match fs::read_to_string(&resp_path) {
            Ok(s) => {
                let trimmed = s.trim().to_string();
                if trimmed.is_empty() {
                    if start.elapsed() > Duration::from_secs(5) {
                        return Err(io::Error::new(
                            io::ErrorKind::TimedOut,
                            "empty passthrough response",
                        ));
                    }
                    std::thread::sleep(Duration::from_millis(10));
                    continue;
                }
                let _ = fs::remove_file(&resp_path);
                let _ = fs::remove_file(&req_path);
                return Ok(trimmed);
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                if start.elapsed() > Duration::from_secs(5) {
                    let _ = fs::remove_file(&req_path);
                    return Err(io::Error::new(io::ErrorKind::TimedOut, "passthrough timeout"));
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(e) => return Err(e),
        }
    }
}

enum PassthroughEndpoint {
    Proc(PathBuf),
    Dir(PathBuf),
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut out, "{b:02x}");
    }
    out
}

fn hex_decode(hex: &str) -> Result<Vec<u8>, String> {
    if hex.len() % 2 != 0 {
        return Err("odd hex length".to_string());
    }
    let mut out = Vec::with_capacity(hex.len() / 2);
    let mut i = 0;
    while i < hex.len() {
        let byte = u8::from_str_radix(&hex[i..i + 2], 16).map_err(|e| e.to_string())?;
        out.push(byte);
        i += 2;
    }
    Ok(out)
}

fn parse_ok_ret(resp: &str) -> Result<u64, String> {
    let mut it = resp.split_whitespace();
    let status = it.next().ok_or_else(|| "missing status".to_string())?;
    if status != "OK" {
        return Err(resp.to_string());
    }
    let ret = it
        .next()
        .ok_or_else(|| "missing ret".to_string())?
        .parse::<u64>()
        .map_err(|e| e.to_string())?;
    Ok(ret)
}

fn parse_ok_ret_and_hex(resp: &str) -> Result<(u64, String), String> {
    let mut it = resp.split_whitespace();
    let status = it.next().ok_or_else(|| "missing status".to_string())?;
    if status != "OK" {
        return Err(resp.to_string());
    }
    let ret = it
        .next()
        .ok_or_else(|| "missing ret".to_string())?
        .parse::<u64>()
        .map_err(|e| e.to_string())?;
    let hex = it.next().ok_or_else(|| "missing hex".to_string())?.to_string();
    Ok((ret, hex))
}
