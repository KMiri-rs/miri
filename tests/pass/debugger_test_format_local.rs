#![allow(dead_code)]

use std::hint::black_box;

fn dummy_fn() {}

fn main() {
    // ty::Bool — must use black_box, otherwise the compiler folds these to constants
    // in var_debug_info and they have no real MIR local slot for the debugger to read.
    let _b_true = black_box(true);
    let _b_false = black_box(false);

    // ty::Char
    let _c = black_box('x');

    // ty::Int / ty::Uint — small (no hex suffix), large (decimal + hex suffix)
    let _i8 = black_box(-1_i8);
    let _i32 = black_box(-42_000_i32);
    let _i64 = black_box(-1_000_000_000_i64);
    let _u8 = black_box(9_u8);
    let _u32 = black_box(100_000_u32);
    let _u64 = black_box(1_000_000_000_u64);
    let _usize = black_box(42_usize);

    // ty::Float
    let _f32 = black_box(3.14_f32);
    let _f64 = black_box(2.718_281_828_f64);

    // ty::Tuple — unit, pair, single-element
    let _unit = black_box(());
    let _pair = (black_box(1_i32), black_box(2_i32));
    let _single = (black_box(42_i32),);

    // ZST Adt (format_zst path)
    let _zst = black_box(Zst);

    // ty::Array
    let _arr = [black_box(1_i32), 2, 3, 4];
    // array longer than MAX_ITEMS to exercise the truncation path
    let _long_arr = [black_box(0_u8); 16];

    // ty::Ref — &str, &[u8], &[T], &T (primitive)
    let _str: &str = black_box("hello world");
    let _bytes: &[u8] = black_box(b"raw bytes");
    let _slice: &[i32] = black_box(&[10, 20, 30]);
    let n = black_box(99_i32);
    let _ref_prim = &n;

    // ty::RawPtr
    let _raw_const: *const i32 = &n as *const i32;
    let _raw_mut: *mut i32 = &n as *const i32 as *mut i32;

    // ty::FnPtr
    let _fp: fn() = black_box(dummy_fn as fn());

    // ty::FnDef (zero-sized function-def type, before any coercion)
    let _fn_def = black_box(dummy_fn);

    // ty::Adt — struct with fields
    let _s = S { a: black_box(3_usize), b: "hello".to_owned() };

    // ty::Adt — unit struct (ZST struct, handled by format_zst via is_zst())
    let _zst2 = black_box(Zst);

    // ty::Adt — union (format shows "<union Name>")
    let _u = U { a: black_box(0xDEAD_BEEFu32) };

    // ty::Adt enum — unit variant, tuple variant, struct variant
    let _e_unit = black_box(E::Unit);
    let _e_tuple = black_box(E::Tuple(1, 2));
    let _e_named = black_box(E::Named { x: 3 });

    // Common stdlib enums
    let _none: Option<i32> = black_box(None);
    let _some = black_box(Some(42_i32));
    let _ok: Result<i32, &str> = black_box(Ok(0));
    let _err: Result<i32, &str> = black_box(Err("oops"));

    // ty::Closure — captures a non-constant local so it is a real closure
    let captured = black_box(7_i32);
    let _cl = black_box(move || captured * 2);
}

struct S {
    a: usize,
    b: String,
}

struct Zst;

enum E {
    Unit,
    Tuple(i32, i32),
    Named { x: i32 },
}

union U {
    a: u32,
    b: i32,
}

// idx=`__1` name=`_b_true` ty=`bool` value=`true
// idx=`__2` name=`_b_false` ty=`bool` value=`false
// idx=`__3` name=`_c` ty=`char` value=`'x'
// idx=`__4` name=`_i8` ty=`i8` value=`-1
// idx=`__5` name=`_i32` ty=`i32` value=`-42000 (0xffffffffffffffffffffffffffff5bf0)
// idx=`__6` name=`_i64` ty=`i64` value=`-1000000000 (0xffffffffffffffffffffffffc4653600)
// idx=`__7` name=`_u8` ty=`u8` value=`9
// idx=`__8` name=`_u32` ty=`u32` value=`100000 (0x186a0)
// idx=`__9` name=`_u64` ty=`u64` value=`1000000000 (0x3b9aca00)
// idx=`__10` name=`_usize` ty=`usize` value=`42 (0x2a)
// idx=`__11` name=`_f32` ty=`f32` value=`3.1400001
// idx=`__12` name=`_f64` ty=`f64` value=`2.7182818279999998
// idx=`__13` name=`_unit` ty=`()` value=`()
// idx=`__15` name=`_pair` ty=`(i32, i32)` value=`(1, 2)
// idx=`__18` name=`_single` ty=`(i32,)` value=`(42 (0x2a),)
// idx=`__20` name=`_zst` ty=`Zst` value=`Zst
// idx=`__22` name=`_arr` ty=`[i32; 4]` value=`[1, 2, 3, 4]
// idx=`__24` name=`_long_arr` ty=`[u8; 16]` value=`[0, 0, 0, 0, 0, 0, 0, 0, ...]
// idx=`__26` name=`_str` ty=`&str` value=`"hello world"
// idx=`__30` name=`_bytes` ty=`&[u8]` value=`[114, 97, 119, 32, 98, 121, 116, 101, ...]
// idx=`__35` name=`_slice` ty=`&[i32]` value=`&[10 (0xa), 20 (0x14), 30 (0x1e)]
// idx=`__41` name=`n` ty=`i32` value=`99 (0x63)
// idx=`__42` name=`_ref_prim` ty=`&i32` value=`&99 (0x63)
// idx=`__43` name=`_raw_const` ty=`*const i32` value=`0xffffffff80fffd94 (alloc179)
// idx=`__45` name=`_raw_mut` ty=`*mut i32` value=`0xffffffff80fffd94 (alloc179)
// idx=`__48` name=`_fp` ty=`fn()` value=`0xffffffff80010251 (alloc180)
// idx=`__50` name=`_fn_def` ty=`fn() {dummy_fn}` value=`fn() {dummy_fn}
// idx=`__51` name=`_s` ty=`S` value=`S { a: 3, b: String { vec: Vec { buf: RawVec { inner: .., _marker: .. }, len: 5 } } }
// idx=`__56` name=`_zst2` ty=`Zst` value=`Zst
// idx=`__58` name=`_u` ty=`U` value=`<union U>
// idx=`__60` name=`_e_unit` ty=`E` value=`E::Unit
// idx=`__62` name=`_e_tuple` ty=`E` value=`E::Tuple(1, 2)
// idx=`__64` name=`_e_named` ty=`E` value=`E::Named { x: 3 }
// idx=`__66` name=`_none` ty=`std::option::Option<i32>` value=`Option::None
// idx=`__68` name=`_some` ty=`std::option::Option<i32>` value=`Option::Some(42 (0x2a))
// idx=`__70` name=`_ok` ty=`std::result::Result<i32, &str>` value=`Result::Ok(0)
// idx=`__72` name=`_err` ty=`std::result::Result<i32, &str>` value=`Result::Err("oops")
// idx=`__76` name=`captured` ty=`i32` value=`7`
// idx=`__77` name=`_cl` ty=`{closure@tests/pass/debugger_test_format_local.rs:81:25: 81:32}` value=`{closure}(7,)`

