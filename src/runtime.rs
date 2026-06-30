#![allow(non_snake_case)]

use crate::{
    starknet::{ArrayAbi, Felt252Abi},
    utils::{blake_utils, BuiltinCosts},
};
use bumpalo::Bump;
use cairo_lang_sierra_gas::core_libfunc_cost::{
    DICT_SQUASH_REPEATED_ACCESS_COST, DICT_SQUASH_UNIQUE_KEY_COST,
};
use itertools::Itertools;
use lambdaworks_math::field::fields::mersenne31::extensions::Degree4ExtensionField;
use lazy_static::lazy_static;
use num_bigint::BigUint;
use num_traits::{ToPrimitive, Zero};
use starknet_curve::curve_params::BETA;
use starknet_types_core::{
    curve::{AffinePoint, ProjectivePoint},
    felt::{Felt, NonZeroFelt},
    hash::StarkHash,
    qm31::QM31,
};
use std::{
    alloc::Layout,
    cell::{Cell, RefCell},
    collections::{hash_map::Entry, HashMap},
    ffi::{c_int, c_void},
    fs::File,
    io::Write,
    mem::ManuallyDrop,
    os::fd::FromRawFd,
    ptr::{self, null_mut},
};
use std::{ops::Mul, vec::IntoIter};

// Thread-local handle to the per-execution arena (boxes, nullables, arrays).
thread_local! {
pub(crate) static EXECUTION_ARENA: RefCell<Bump> = RefCell::new(Bump::new());

    /// Registry of all FeltDicts created during the current invocation.
    /// Drained during InvocationGuard drop to drop HashMaps (which live on the
    /// system heap and cannot be reclaimed by the arena).
    pub(crate) static DICT_REGISTRY: RefCell<Vec<*mut FeltDict>> = const { RefCell::new(Vec::new()) };
}

/// Compute `floor(sqrt(value))`. The result of each integer square root always
/// fits in the (smaller) output type used by the corresponding libfunc.
pub extern "C" fn cairo_native__u8_square_root(value: u8) -> u8 {
    value.isqrt()
}

/// Compute `floor(sqrt(value))`. See [`cairo_native__u8_square_root`].
pub extern "C" fn cairo_native__u16_square_root(value: u16) -> u8 {
    value.isqrt() as u8
}

/// Compute `floor(sqrt(value))`. See [`cairo_native__u8_square_root`].
pub extern "C" fn cairo_native__u32_square_root(value: u32) -> u16 {
    value.isqrt() as u16
}

/// Compute `floor(sqrt(value))`. See [`cairo_native__u8_square_root`].
pub extern "C" fn cairo_native__u64_square_root(value: u64) -> u32 {
    value.isqrt() as u32
}

/// Compute `floor(sqrt(value))`. See [`cairo_native__u8_square_root`].
pub extern "C" fn cairo_native__u128_square_root(value: u128) -> u64 {
    value.isqrt() as u64
}

/// Compute `floor(sqrt(value))` of the `u256` given by its low and high `u128`
/// limbs. The result always fits in a `u128`.
pub extern "C" fn cairo_native__u256_square_root(lo: u128, hi: u128) -> u128 {
    let value = (BigUint::from(hi) << 128u32) + BigUint::from(lo);
    value
        .sqrt()
        .to_u128()
        .expect("the square root of a u256 always fits in a u128")
}

/// Compute `(lhs * rhs) mod STARK_PRIME` for two felt252 values, each given as
/// a 32-byte little-endian buffer, writing the field product to `dst`.
///
/// felt252 values are stored canonically. A naive `Felt::from_bytes_le(lhs) *
/// Felt::from_bytes_le(rhs)` then `.to_bytes_le()` costs four Montgomery
/// multiplications: two canonical->Montgomery conversions on the inputs, the
/// multiply, and one Montgomery->canonical conversion on the output.
///
/// We cut that to two by exploiting that `Felt::from_raw`/`to_raw` are free
/// (they reinterpret the limbs as the internal Montgomery representation,
/// without any conversion):
/// - `from_raw(canonical(a))` yields a `Felt` whose *value* is `a * R⁻¹`,
/// - multiplying it by `from_bytes_le(b)` (value `b`) gives value `a * b * R⁻¹`,
///   whose raw representation is exactly `a * b`,
/// - so `to_raw` reads the canonical product straight out.
///
/// Only the `from_bytes_le(rhs)` conversion and the multiply itself cost a
/// Montgomery multiplication.
pub extern "C" fn cairo_native__felt252_mul(dst: &mut [u8; 32], lhs: &[u8; 32], rhs: &[u8; 32]) {
    // value = a * R⁻¹ (free: reinterpret canonical bytes as raw limbs).
    let lhs = felt_from_raw_le_bytes(lhs);
    // value = b (one Montgomery multiplication).
    let rhs = Felt::from_bytes_le(rhs);
    // value = a * b * R⁻¹, so its raw representation is the canonical `a * b`.
    *dst = felt_raw_to_le_bytes(&(lhs * rhs));
}

/// Build a `Felt` by interpreting a 32-byte little-endian buffer as the felt's
/// raw internal (Montgomery) representation. The inverse of
/// [`felt_raw_to_le_bytes`]. Zero-cost: no canonical<->Montgomery conversion.
///
/// `Felt::from_raw` takes limbs most-significant-first, while the buffer is
/// little-endian, so the limbs are read least-significant-first then reversed.
fn felt_from_raw_le_bytes(buffer: &[u8; 32]) -> Felt {
    let mut limbs = [0u64; 4];
    for (i, limb) in limbs.iter_mut().enumerate() {
        *limb = u64::from_le_bytes(buffer[i * 8..i * 8 + 8].try_into().unwrap());
    }
    limbs.reverse();
    Felt::from_raw(limbs)
}

/// Little-endian image of a `Felt`'s raw internal (Montgomery) representation.
/// The inverse of [`felt_from_raw_le_bytes`]. Zero-cost.
fn felt_raw_to_le_bytes(value: &Felt) -> [u8; 32] {
    let limbs = value.to_raw_reversed(); // least-significant limb first
    let mut buffer = [0u8; 32];
    for (i, limb) in limbs.iter().enumerate() {
        buffer[i * 8..i * 8 + 8].copy_from_slice(&limb.to_le_bytes());
    }
    buffer
}

/// Compute `lhs / rhs` in the STARK field, i.e. `lhs * rhs⁻¹ mod STARK_PRIME`,
/// and store the canonical little-endian result in `dst`.
///
/// `rhs` originates from a `NonZero<felt252>`, so it is guaranteed nonzero;
/// `field_div` uses the optimized Montgomery inverse.
pub extern "C" fn cairo_native__felt252_div(dst: &mut [u8; 32], lhs: &[u8; 32], rhs: &[u8; 32]) {
    let lhs = Felt::from_bytes_le(lhs);
    let rhs = NonZeroFelt::from_felt_unchecked(Felt::from_bytes_le(rhs));
    *dst = lhs.field_div(&rhs).to_bytes_le();
}

/// Allocate `size` bytes with `align` alignment from the per-execution arena.
pub unsafe extern "C" fn cairo_native__arena_alloc(size: u64, align: u64) -> *mut u8 {
    EXECUTION_ARENA.with(|arena| {
        let layout = Layout::from_size_align(size as usize, align as usize)
            .expect("cairo_native__arena_alloc: invalid layout");
        arena.borrow_mut().alloc_layout(layout).as_ptr()
    })
}

lazy_static! {
    pub static ref HALF_PRIME: Felt = Felt::from_dec_str(
        "1809251394333065606848661391547535052811553607665798349986546028067936010240"
    )
    .unwrap();
    pub static ref DICT_GAS_REFUND_PER_ACCESS: u64 =
        (DICT_SQUASH_UNIQUE_KEY_COST.cost() - DICT_SQUASH_REPEATED_ACCESS_COST.cost()) as u64;
}

/// Based on `cairo-lang-runner`'s implementation.
///
/// Source: <https://github.com/starkware-libs/cairo/blob/main/crates/cairo-lang-runner/src/casm_run/mod.rs#L1946-L1948>
///
/// # Safety
///
/// This function is intended to be called from MLIR, deals with pointers, and is therefore
/// definitely unsafe to use manually.
pub unsafe extern "C" fn cairo_native__libfunc__debug__print(
    target_fd: i32,
    data: *const [u8; 32],
    len: u32,
) -> i32 {
    // Avoid closing `stdout` on all branches.
    let mut target = ManuallyDrop::new(File::from_raw_fd(target_fd));

    let mut items = Vec::with_capacity(len as usize);

    for i in 0..len as usize {
        let mut data = *data.add(i);
        data[31] &= 0x0F; // Filter out first 4 bits (they're outside an i252).

        let value = Felt::from_bytes_le(&data);
        items.push(value);
    }

    let value = format_for_debug(items.into_iter());

    if write!(target, "{}", value).is_err() {
        return 1;
    };

    0
}

/// Compute `pedersen(lhs, rhs)` and store it into `dst`.
///
/// All its operands need the values in big endian.
///
/// # Panics
///
/// This function will panic if either operand is out of range for a felt.
///
/// # Safety
///
/// This function is intended to be called from MLIR, deals with pointers, and is therefore
/// definitely unsafe to use manually.
pub unsafe extern "C" fn cairo_native__libfunc__pedersen(
    dst: &mut [u8; 32],
    lhs: &[u8; 32],
    rhs: &[u8; 32],
) {
    // Extract arrays from the pointers.
    let mut lhs = *lhs;
    let mut rhs = *rhs;

    lhs[31] &= 0x0F; // Filter out first 4 bits (they're outside an i252).
    rhs[31] &= 0x0F; // Filter out first 4 bits (they're outside an i252).

    // Convert to FieldElement.
    let lhs = Felt::from_bytes_le(&lhs);
    let rhs = Felt::from_bytes_le(&rhs);

    // Compute pedersen hash and copy the result into `dst`.
    let res = starknet_types_core::hash::Pedersen::hash(&lhs, &rhs);
    *dst = res.to_bytes_le();
}

/// Compute `hades_permutation(op0, op1, op2)` and replace the operands with the results.
///
/// All operands need the values in big endian.
///
/// # Panics
///
/// This function will panic if either operand is out of range for a felt.
///
/// # Safety
///
/// This function is intended to be called from MLIR, deals with pointers, and is therefore
/// definitely unsafe to use manually.
pub unsafe extern "C" fn cairo_native__libfunc__hades_permutation(
    op0: &mut [u8; 32],
    op1: &mut [u8; 32],
    op2: &mut [u8; 32],
) {
    op0[31] &= 0x0F; // Filter out first 4 bits (they're outside an i252).
    op1[31] &= 0x0F; // Filter out first 4 bits (they're outside an i252).
    op2[31] &= 0x0F; // Filter out first 4 bits (they're outside an i252).

    // Convert to FieldElement.
    let mut state = [
        Felt::from_bytes_le(op0),
        Felt::from_bytes_le(op1),
        Felt::from_bytes_le(op2),
    ];

    // Compute Poseidon permutation.
    starknet_types_core::hash::Poseidon::hades_permutation(&mut state);

    // Write back the results.
    *op0 = state[0].to_bytes_le();
    *op1 = state[1].to_bytes_le();
    *op2 = state[2].to_bytes_le();
}

pub unsafe extern "C" fn cairo_native__libfunc__blake_compress(
    out_state: &mut [u32; 8],
    state: &[u32; 8],
    message: &[u32; 16],
    count_bytes: u32,
    finalize: bool,
) {
    let new_state = blake_utils::blake2s_compress(
        state,
        message,
        count_bytes,
        0,
        if finalize { 0xFFFFFFFF } else { 0 },
        0,
    );

    *out_state = new_state;

    // Track blake invocations: Blake doesn't have an implicit counter argument
    // like buffer-based builtins, so we count calls here directly.
    BLAKE_CALL_COUNT.with(|c| c.set(c.get() + 1));
}

/// Felt252 type used in cairo native runtime
#[derive(Debug)]
pub struct FeltDict {
    pub mappings: HashMap<Felt, usize>,

    pub layout: Layout,
    pub elements: *mut (),

    pub count: u64,
}

// No Drop impl — the arena owns the FeltDict struct and elements buffer.
// HashMaps are cleaned up via DICT_REGISTRY during arena reset.

/// Allocate a new dictionary.
///
/// # Safety
///
/// This function is intended to be called from MLIR, deals with pointers, and is therefore
/// definitely unsafe to use manually.
pub unsafe extern "C" fn cairo_native__dict_new(size: u64, align: u64) -> *mut FeltDict {
    let dict_ptr = EXECUTION_ARENA.with(|arena| {
        let layout = Layout::new::<FeltDict>();
        arena.borrow_mut().alloc_layout(layout).as_ptr() as *mut FeltDict
    });

    dict_ptr.write(FeltDict {
        mappings: HashMap::default(),
        layout: Layout::from_size_align_unchecked(size as usize, align as usize),
        elements: ptr::null_mut(),
        count: 0,
    });

    DICT_REGISTRY.with(|reg| reg.borrow_mut().push(dict_ptr));

    dict_ptr
}

/// Return a pointer to the entry's value pointer for a given key, inserting a null pointer if not
/// present. Increment the access count.
///
/// The null pointer will be either updated by `felt252_dict_entry_finalize` or removed (along with
/// everything else in the dict) by the entry's drop implementation.
///
/// # Safety
///
/// This function is intended to be called from MLIR, deals with pointers, and is therefore
/// definitely unsafe to use manually.
pub unsafe extern "C" fn cairo_native__dict_get(
    dict_ptr: *mut FeltDict,
    key: &[u8; 32],
    value_ptr: *mut *mut c_void,
) -> c_int {
    let dict = &mut *dict_ptr;

    let num_mappings = dict.mappings.len();
    let has_capacity = num_mappings != dict.mappings.capacity();

    let (is_present, index) = match dict.mappings.entry(Felt::from_bytes_le(key)) {
        Entry::Occupied(entry) => (true, *entry.get()),
        Entry::Vacant(entry) => {
            entry.insert(num_mappings);
            (false, num_mappings)
        }
    };

    // Grow the elements buffer if the HashMap grew its capacity.
    if !has_capacity && !is_present {
        let elem_stride = dict.layout.pad_to_align().size();
        let old_size = elem_stride * num_mappings;
        let new_size = elem_stride * dict.mappings.capacity();

        dict.elements = EXECUTION_ARENA.with(|arena| {
            let layout = Layout::from_size_align_unchecked(new_size, dict.layout.align());
            let new_ptr = arena
                .borrow_mut()
                .alloc_layout(layout)
                .as_ptr()
                .cast::<()>();
            if !dict.elements.is_null() && old_size > 0 {
                std::ptr::copy_nonoverlapping(
                    dict.elements.cast::<u8>(),
                    new_ptr.cast::<u8>(),
                    old_size,
                );
            }
            new_ptr
        });
    }

    *value_ptr = dict
        .elements
        .byte_add(dict.layout.pad_to_align().size() * index)
        .cast();

    dict.count += 1;

    is_present as c_int
}

/// Creates an array (Array<(felt252, T, T)>) by iterating the dictionary.
unsafe fn create_dict_entries_array(dict: &mut FeltDict) -> ArrayAbi<c_void> {
    let len = dict.mappings.len();
    if len == 0 {
        return ArrayAbi {
            ptr: null_mut(),
            since: 0,
            until: 0,
            capacity: 0,
        };
    }

    // Get elements sizes for memory allocation
    let tuple_layout = Layout::new::<Felt252Abi>()
        .extend(dict.layout)
        .expect("Should be posible to extend Felt252Abi layout")
        .0
        .extend(dict.layout)
        .expect("Should be able to extend with the last tuple element")
        .0;
    let tuple_stride = tuple_layout.pad_to_align().size();

    // Allocate data from the arena (no inline prefix)
    let data_ptr = cairo_native__arena_alloc(
        (tuple_stride * dict.mappings.len()) as u64,
        tuple_layout.align() as u64,
    );
    let mut work_ptr = data_ptr;

    // Get the stride for the inner types of the tuple
    let key_size = Layout::new::<Felt252Abi>().pad_to_align().size();
    let element_size = dict.layout.pad_to_align().size();

    for (key, elem_index) in dict.mappings.iter().sorted() {
        let key_ptr = work_ptr as *mut [u8; 32];
        let default_value_ptr = work_ptr.byte_add(key_size);
        let final_value_ptr = default_value_ptr.byte_add(element_size);
        work_ptr = work_ptr.byte_add(tuple_stride);

        *key_ptr = key.to_bytes_le();
        default_value_ptr.write_bytes(0, element_size);
        let value = dict.elements.byte_add(element_size * elem_index) as *mut u8;
        final_value_ptr.copy_from_nonoverlapping(value, element_size);
    }

    ArrayAbi {
        ptr: data_ptr.cast(),
        since: 0,
        until: len as u32,
        capacity: len as u32,
    }
}

/// Fills each of the tuples in the array with the corresponding content.
///
/// Receives a pointer to the dictionary and moves its entries into the given uninitialized array of
/// (felt252, T, T) tuples.  The dictionary is iterated and for each element, a tuple is filled with the key
/// and the value.
///
/// # Caveats
///
/// Each tuple has the form (felt252, T, T) = (key, first_value, last_value). 'last_value' is represents
/// the value of the element in the dictionary and 'first_value' is always the zero-value of T.
pub unsafe extern "C" fn cairo_native__dict_into_entries(
    dict_ptr: *mut FeltDict,
    array_ptr: *mut ArrayAbi<c_void>,
) {
    let dict = &mut *dict_ptr;

    let arr = create_dict_entries_array(dict);
    *array_ptr = arr;
}

/// Simulates the felt252_dict_squash libfunc.
///
/// # Safety
///
/// This function is intended to be called from MLIR, deals with pointers, and is therefore
/// definitely unsafe to use manually.
pub unsafe extern "C" fn cairo_native__dict_squash(
    dict_ptr: *const FeltDict,
    range_check_ptr: &mut u64,
    gas_ptr: &mut u64,
) {
    let dict = &*dict_ptr;

    *gas_ptr +=
        (dict.count.saturating_sub(dict.mappings.len() as u64)) * *DICT_GAS_REFUND_PER_ACCESS;

    // Squashing a dictionary always uses the range check builtin at least twice.
    // https://github.com/starkware-libs/cairo/blob/v2.12.0-dev.1/crates/cairo-lang-sierra-to-casm/src/invocations/felt252_dict.rs?plain=1#L131-L136
    *range_check_ptr += 2;

    let u128_max = Felt::from(u128::MAX);
    let no_big_keys = dict.mappings.keys().all(|key| *key <= u128_max);
    let number_of_keys = dict.mappings.len() as u64;

    // How we update the range check depends on whether we have any big key or not.
    // - If there are no big keys, every unique key increases the range check by 3.
    // - If there are big keys:
    //   - the first unique key increases the range check by 2.
    //   - the remaining unique keys increase the range check by 6.

    // The sierra-to-casm implementation calls the `SquashDict` after some initial validation.
    // https://github.com/starkware-libs/cairo/blob/v2.12.0-dev.1/crates/cairo-lang-sierra-to-casm/src/invocations/felt252_dict.rs?plain=1#L159
    //
    // For each unique key, the `SquashDictInner` function is called, which
    // loops over all accesses to that key. At the end, the function calls
    // itself recursively until all keys have been iterated.

    // If there are no big keys, the first range check usage is done by the
    // caller of the inner function, which implies that it appears in two places:
    // 1a. Once in `SquashDict`, right before calling the inner function for the first time.
    //     https://github.com/starkware-libs/cairo/blob/v2.12.0-dev.1/crates/cairo-lang-sierra-to-casm/src/invocations/felt252_dict.rs?plain=1#L326
    // 1b. Once at the end of `SquashDictInner`, right before recursing.
    //     https://github.com/starkware-libs/cairo/blob/v2.12.0-dev.1/crates/cairo-lang-sierra-to-casm/src/invocations/felt252_dict.rs?plain=1#L507
    if no_big_keys {
        *range_check_ptr += number_of_keys;
    }

    // The next two range check usages are done always inside of the inner
    // function (regardless of whether we have big keys or not).
    // 2.  https://github.com/starkware-libs/cairo/blob/v2.12.0-dev.1/crates/cairo-lang-sierra-to-casm/src/invocations/felt252_dict.rs?plain=1#L416
    // 3.  https://github.com/starkware-libs/cairo/blob/v2.12.0-dev.1/crates/cairo-lang-sierra-to-casm/src/invocations/felt252_dict.rs?plain=1#L480
    *range_check_ptr += 2 * number_of_keys;

    // If there are big keys, then we use the range check 4 additional times per key, except for the first key.
    // https://github.com/starkware-libs/cairo/blob/v2.12.0-dev.1/crates/cairo-lang-sierra-to-casm/src/invocations/felt252_dict.rs#L669-L674
    if !no_big_keys && number_of_keys > 1 {
        *range_check_ptr += 4 * (number_of_keys - 1);
    }

    // For each non unique accessed key, we increase the range check an additional time.
    // https://github.com/starkware-libs/cairo/blob/v2.12.0-dev.1/crates/cairo-lang-sierra-to-casm/src/invocations/felt252_dict.rs?plain=1#L602
    *range_check_ptr += dict.count.saturating_sub(dict.mappings.len() as u64);
}

/// Compute `ec_point_from_x_nz(x)` and store it.
///
/// # Panics
///
/// This function will panic if either operand is out of range for a felt.
///
/// # Safety
///
/// This function is intended to be called from MLIR, deals with pointers, and is therefore
/// definitely unsafe to use manually.
pub unsafe extern "C" fn cairo_native__libfunc__ec__ec_point_from_x_nz(
    point_ptr: &mut [[u8; 32]; 2],
) -> bool {
    point_ptr[0][31] &= 0x0F; // Filter out first 4 bits (they're outside an i252).
    let x = Felt::from_bytes_le(&point_ptr[0]);

    // https://github.com/starkware-libs/cairo/blob/aaad921bba52e729dc24ece07fab2edf09ccfa15/crates/cairo-lang-sierra-to-casm/src/invocations/ec.rs#L63

    let x2 = x * x;
    let x3 = x2 * x;
    let alpha_x_plus_beta = x + BETA;
    let rhs = x3 + alpha_x_plus_beta;
    // https://github.com/starkware-libs/cairo/blob/9b603b88c2e5a98eec1bb8f323260b7765e94911/crates/cairo-lang-runner/src/casm_run/mod.rs#L1825
    let y = rhs
        .sqrt()
        .unwrap_or_else(|| (Felt::THREE * rhs).sqrt().unwrap());
    let y = y.min(-y);

    match AffinePoint::new(x, y) {
        Ok(point) => {
            point_ptr[1] = point.y().to_bytes_le();
            true
        }
        Err(_) => false,
    }
}

/// Compute `ec_point_try_new_nz(x)`.
///
/// # Panics
///
/// This function will panic if either operand is out of range for a felt.
///
/// # Safety
///
/// This function is intended to be called from MLIR, deals with pointers, and is therefore
/// definitely unsafe to use manually.
pub unsafe extern "C" fn cairo_native__libfunc__ec__ec_point_try_new_nz(
    point_ptr: &mut [[u8; 32]; 2],
) -> bool {
    point_ptr[0][31] &= 0x0F; // Filter out first 4 bits (they're outside an i252).
    point_ptr[1][31] &= 0x0F; // Filter out first 4 bits (they're outside an i252).

    let x = Felt::from_bytes_le(&point_ptr[0]);
    let y = Felt::from_bytes_le(&point_ptr[1]);

    match AffinePoint::new(x, y) {
        Ok(point) => {
            point_ptr[0] = point.x().to_bytes_le();
            point_ptr[1] = point.y().to_bytes_le();
            true
        }
        Err(_) => false,
    }
}

/// Compute `ec_state_add(state, point)` and store the state back.
///
/// # Panics
///
/// This function will panic if either operand is out of range for a felt.
///
/// # Safety
///
/// This function is intended to be called from MLIR, deals with pointers, and is therefore
/// definitely unsafe to use manually.
pub unsafe extern "C" fn cairo_native__libfunc__ec__ec_state_add(
    state_ptr: &mut [[u8; 32]; 2],
    point_ptr: &[[u8; 32]; 2],
) {
    state_ptr[0][31] &= 0x0F; // Filter out first 4 bits (they're outside an i252).
    state_ptr[1][31] &= 0x0F; // Filter out first 4 bits (they're outside an i252).

    let x = Felt::from_bytes_le(&state_ptr[0]);
    let y = Felt::from_bytes_le(&state_ptr[1]);
    if x.is_zero() && y.is_zero() {
        *state_ptr = *point_ptr;
        return;
    }

    let mut point_ptr = *point_ptr;
    point_ptr[0][31] &= 0x0F; // Filter out first 4 bits (they're outside an i252).
    point_ptr[1][31] &= 0x0F; // Filter out first 4 bits (they're outside an i252).
                              // We use unchecked methods because the inputs must already be valid points.
    let mut state = ProjectivePoint::from_affine_unchecked(x, y);
    let point = AffinePoint::new_unchecked(
        Felt::from_bytes_le(&point_ptr[0]),
        Felt::from_bytes_le(&point_ptr[1]),
    );

    state += &point;
    match state.to_affine() {
        Ok(state) => {
            state_ptr[0] = state.x().to_bytes_le();
            state_ptr[1] = state.y().to_bytes_le();
        }
        Err(_) => *state_ptr = [[0u8; 32]; 2],
    }
}

/// Compute `ec_state_add_mul(state, scalar, point)` and store the state back.
///
/// # Panics
///
/// This function will panic if either operand is out of range for a felt.
///
/// # Safety
///
/// This function is intended to be called from MLIR, deals with pointers, and is therefore
/// definitely unsafe to use manually.
pub unsafe extern "C" fn cairo_native__libfunc__ec__ec_state_add_mul(
    state_ptr: &mut [[u8; 32]; 2],
    scalar_ptr: &[u8; 32],
    point_ptr: &[[u8; 32]; 2],
) {
    state_ptr[0][31] &= 0x0F; // Filter out first 4 bits (they're outside an i252).
    state_ptr[1][31] &= 0x0F; // Filter out first 4 bits (they're outside an i252).
    let x = Felt::from_bytes_le(&state_ptr[0]);
    let y = Felt::from_bytes_le(&state_ptr[1]);

    let mut point_ptr = *point_ptr;
    point_ptr[0][31] &= 0x0F; // Filter out first 4 bits (they're outside an i252).
    point_ptr[1][31] &= 0x0F; // Filter out first 4 bits (they're outside an i252).

    let mut scalar_ptr = *scalar_ptr;
    scalar_ptr[31] &= 0x0F; // Filter out first 4 bits (they're outside an i252).

    // Here the points should already be checked as valid, so we can use unchecked.
    let mut state = if x.is_zero() && y.is_zero() {
        ProjectivePoint::identity()
    } else {
        ProjectivePoint::from_affine_unchecked(x, y)
    };
    let point = ProjectivePoint::from_affine_unchecked(
        Felt::from_bytes_le(&point_ptr[0]),
        Felt::from_bytes_le(&point_ptr[1]),
    );
    let scalar = Felt::from_bytes_le(&scalar_ptr);

    state += &point.mul(scalar);
    match state.to_affine() {
        Ok(state) => {
            state_ptr[0] = state.x().to_bytes_le();
            state_ptr[1] = state.y().to_bytes_le();
        }
        Err(_) => *state_ptr = [[0u8; 32]; 2],
    }
}

/// Compute `ec_state_try_finalize_nz(state)` and store the result.
///
/// # Panics
///
/// This function will panic if either operand is out of range for a felt.
///
/// # Safety
///
/// This function is intended to be called from MLIR, deals with pointers, and is therefore
/// definitely unsafe to use manually.
pub unsafe extern "C" fn cairo_native__libfunc__ec__ec_state_try_finalize_nz(
    point_ptr: &mut [[u8; 32]; 2],
    state_ptr: &[[u8; 32]; 2],
) -> bool {
    let mut state_ptr = *state_ptr;
    state_ptr[0][31] &= 0x0F; // Filter out first 4 bits (they're outside an i252).
    state_ptr[1][31] &= 0x0F; // Filter out first 4 bits (they're outside an i252).
    let x = Felt::from_bytes_le(&state_ptr[0]);
    let y = Felt::from_bytes_le(&state_ptr[1]);
    if x.is_zero() && y.is_zero() {
        false
    } else {
        *point_ptr = state_ptr;
        true
    }
}

/// Compute `qm31_add(qm31, qm31)` and store the result.
///
/// # Safety
///
/// This function is intended to be called from MLIR, deals with pointers, and is therefore
/// definitely unsafe to use manually.
pub unsafe extern "C" fn cairo_native__libfunc__qm31__qm31_add(
    lhs: &[u32; 4],
    rhs: &[u32; 4],
    res: &mut [u32; 4],
) {
    // We can use this way of creating the QM31 since we already know from cairo that the
    // coefficients will never be more than 31 bits wide
    let lhs = QM31(Degree4ExtensionField::const_from_coefficients(
        lhs[0], lhs[1], lhs[2], lhs[3],
    ));
    let rhs = QM31(Degree4ExtensionField::const_from_coefficients(
        rhs[0], rhs[1], rhs[2], rhs[3],
    ));

    *res = qm31_to_representative_coefficients(lhs + rhs);
}

/// Compute `qm31_sub(qm31, qm31)` and store the result.
///
/// # Safety
///
/// This function is intended to be called from MLIR, deals with pointers, and is therefore
/// definitely unsafe to use manually.
pub unsafe extern "C" fn cairo_native__libfunc__qm31__qm31_sub(
    lhs: &[u32; 4],
    rhs: &[u32; 4],
    res: &mut [u32; 4],
) {
    // We can use this way of creating the QM31 since we already know from cairo that the
    // coefficients will never be more than 31 bits wide
    let lhs = QM31(Degree4ExtensionField::const_from_coefficients(
        lhs[0], lhs[1], lhs[2], lhs[3],
    ));
    let rhs = QM31(Degree4ExtensionField::const_from_coefficients(
        rhs[0], rhs[1], rhs[2], rhs[3],
    ));

    *res = qm31_to_representative_coefficients(lhs - rhs);
}

/// Compute `qm31_mul(qm31, qm31)` and store the result.
///
/// # Safety
///
/// This function is intended to be called from MLIR, deals with pointers, and is therefore
/// definitely unsafe to use manually.
pub unsafe extern "C" fn cairo_native__libfunc__qm31__qm31_mul(
    lhs: &[u32; 4],
    rhs: &[u32; 4],
    res: &mut [u32; 4],
) {
    // We can use this way of creating the QM31 since we already know from cairo that the
    // coefficients will never be more than 31 bits wide
    let lhs = QM31(Degree4ExtensionField::const_from_coefficients(
        lhs[0], lhs[1], lhs[2], lhs[3],
    ));
    let rhs = QM31(Degree4ExtensionField::const_from_coefficients(
        rhs[0], rhs[1], rhs[2], rhs[3],
    ));

    *res = qm31_to_representative_coefficients(lhs * rhs);
}

/// Compute `qm31_div(qm31, qm31)` and store the result.
///
/// # Safety
///
/// This function is intended to be called from MLIR, deals with pointers, and is therefore
/// definitely unsafe to use manually.
pub unsafe extern "C" fn cairo_native__libfunc__qm31__qm31_div(
    lhs: &[u32; 4],
    rhs: &[u32; 4],
    res: &mut [u32; 4],
) {
    // We can use this way of creating the QM31 since we already know from cairo that the
    // coefficients will never be more than 31 bits wide
    let lhs = QM31(Degree4ExtensionField::const_from_coefficients(
        lhs[0], lhs[1], lhs[2], lhs[3],
    ));
    let rhs = QM31(Degree4ExtensionField::const_from_coefficients(
        rhs[0], rhs[1], rhs[2], rhs[3],
    ));

    // SAFETY: An error would be triggered here only if rhs is zero. However, in the QM31 division libfunc, the divisor
    // is of type NonZero<qm31> which ensures that we are not falling into the error case.
    *res = qm31_to_representative_coefficients((lhs / rhs).expect("rhs should not be a QM31 0"));
}

thread_local! {
    pub(crate) static BUILTIN_COSTS: Cell<BuiltinCosts> = const {
        // These default values shouldn't be accessible, they will be overriden before entering
        // compiled code.
        Cell::new(BuiltinCosts {
            r#const: 0,
            pedersen: 0,
            bitwise: 0,
            ecop: 0,
            poseidon: 0,
            add_mod: 0,
            mul_mod: 0,
            blake: 0,
        })
    };

    /// Global counter for blake builtin calls.
    /// Unlike buffer-based builtins (Pedersen, etc.), Blake is a VM opcode without
    /// an implicit counter argument. This global counter is incremented by the
    /// Blake libfuncs (blake2s_compress, blake2s_finalize) on each invocation.
    pub(crate) static BLAKE_CALL_COUNT: Cell<u64> = const { Cell::new(0) };
}

// TODO: This is already implemented on types-rs but there is no release
// that contains it. It should be deleted when bumping to a new version
// and use the .to_coefficients() method from QM31 instead.
pub fn qm31_to_representative_coefficients(qm31: QM31) -> [u32; 4] {
    // Take CM31 coordinates from QM31.
    let [a, b] = qm31.0.value();

    // Take M31 coordinates from both CM31.
    let [c1, c2] = a.value();
    let [c3, c4] = b.value();

    [
        c1.representative(),
        c2.representative(),
        c3.representative(),
        c4.representative(),
    ]
}

/// Get the costs builtin from the internal thread local.
pub extern "C" fn cairo_native__get_costs_builtin() -> *const [u64; 8] {
    BUILTIN_COSTS.with(|x| x.as_ptr()) as *const [u64; 8]
}

// Utility methods for the print runtime function

/// Formats the given felts as a debug string.
fn format_for_debug(mut felts: IntoIter<Felt>) -> String {
    let mut items = Vec::new();
    while let Some(item) = format_next_item(&mut felts) {
        items.push(item);
    }
    if let [item] = &items[..] {
        if item.is_string {
            return item.item.clone();
        }
    }
    items
        .into_iter()
        .map(|item| {
            if item.is_string {
                format!("{}\n", item.item)
            } else {
                format!("[DEBUG]\t{}\n", item.item)
            }
        })
        .join("")
}

/// A formatted string representation of anything formattable (e.g. ByteArray, felt, short-string).
pub struct FormattedItem {
    /// The formatted string representing the item.
    item: String,
    /// Whether the item is a string.
    is_string: bool,
}
impl FormattedItem {
    /// Returns the formatted item as is.
    #[must_use]
    pub fn get(self) -> String {
        self.item
    }
    /// Wraps the formatted item with quote, if it's a string. Otherwise returns it as is.
    #[must_use]
    pub fn quote_if_string(self) -> String {
        if self.is_string {
            format!("\"{}\"", self.item)
        } else {
            self.item
        }
    }
}

pub const BYTE_ARRAY_MAGIC: &str =
    "46a6158a16a947e5916b2a2ca68501a45e93d7110e81aa2d6438b1c57c879a3";
pub const BYTES_IN_WORD: usize = 31;

/// Formats a string or a short string / `felt252`. Returns the formatted string and a boolean
/// indicating whether it's a string. If can't format the item, returns None.
pub fn format_next_item<T>(values: &mut T) -> Option<FormattedItem>
where
    T: Iterator<Item = Felt> + Clone,
{
    let first_felt = values.next()?;

    if first_felt == Felt::from_hex(BYTE_ARRAY_MAGIC).unwrap() {
        if let Some(string) = try_format_string(values) {
            return Some(FormattedItem {
                item: string,
                is_string: true,
            });
        }
    }
    Some(FormattedItem {
        item: format_short_string(&first_felt),
        is_string: false,
    })
}

/// Formats a `Felt252`, as a short string if possible.
fn format_short_string(value: &Felt) -> String {
    let hex_value = value.to_biguint();
    match as_cairo_short_string(value) {
        Some(as_string) => format!("{hex_value:#x} ('{as_string}')"),
        None => format!("{hex_value:#x}"),
    }
}

/// Tries to format a string, represented as a sequence of `Felt252`s.
/// If the sequence is not a valid serialization of a `ByteArray`, returns None and doesn't change the
/// given iterator (`values`).
fn try_format_string<T>(values: &mut T) -> Option<String>
where
    T: Iterator<Item = Felt> + Clone,
{
    // Clone the iterator and work with the clone. If the extraction of the string is successful,
    // change the original iterator to the one we worked with. If not, continue with the
    // original iterator at the original point.
    let mut cloned_values_iter = values.clone();

    let num_full_words = cloned_values_iter.next()?.to_usize()?;
    let full_words = cloned_values_iter
        .by_ref()
        .take(num_full_words)
        .collect_vec();
    let pending_word = cloned_values_iter.next()?;
    let pending_word_len = cloned_values_iter.next()?.to_usize()?;

    let full_words_string = full_words
        .into_iter()
        .map(|word| as_cairo_short_string_ex(&word, BYTES_IN_WORD))
        .collect::<Option<Vec<String>>>()?
        .join("");
    let pending_word_string = as_cairo_short_string_ex(&pending_word, pending_word_len)?;

    // Extraction was successful, change the original iterator to the one we worked with.
    *values = cloned_values_iter;

    Some(format!("{full_words_string}{pending_word_string}"))
}

/// Converts a bigint representing a felt252 to a Cairo short-string.
#[must_use]
pub fn as_cairo_short_string(value: &Felt) -> Option<String> {
    let mut as_string = String::default();
    let mut is_end = false;
    for byte in value.to_biguint().to_bytes_be() {
        if byte == 0 {
            is_end = true;
        } else if is_end {
            return None;
        } else if byte.is_ascii_graphic() || byte.is_ascii_whitespace() {
            as_string.push(byte as char);
        } else {
            return None;
        }
    }
    Some(as_string)
}

/// Converts a bigint representing a felt252 to a Cairo short-string of the given length.
/// Nulls are allowed and length must be <= 31.
#[must_use]
pub fn as_cairo_short_string_ex(value: &Felt, length: usize) -> Option<String> {
    if length == 0 {
        return if value.is_zero() {
            Some(String::new())
        } else {
            None
        };
    }
    if length > 31 {
        // A short string can't be longer than 31 bytes.
        return None;
    }

    // We pass through biguint as felt252.to_bytes_be() does not trim leading zeros.
    let bytes = value.to_biguint().to_bytes_be();
    let bytes_len = bytes.len();
    if bytes_len > length {
        // `value` has more bytes than expected.
        return None;
    }

    let mut as_string = String::new();
    for byte in bytes {
        if byte == 0 {
            as_string.push_str(r"\0");
        } else if byte.is_ascii_graphic() || byte.is_ascii_whitespace() {
            as_string.push(byte as char);
        } else {
            as_string.push_str(format!(r"\x{:02x}", byte).as_str());
        }
    }

    // `to_bytes_be` misses starting nulls. Prepend them as needed.
    let missing_nulls = length - bytes_len;
    as_string.insert_str(0, &r"\0".repeat(missing_nulls));

    Some(as_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        env, fs,
        io::{Read, Seek},
        os::fd::AsRawFd,
    };

    /// The 2-CIOS `cairo_native__felt252_mul` must equal the canonical field
    /// product for every input, including 0, 1, -1, and `PRIME`-adjacent values.
    #[test]
    fn felt252_mul_matches_field_product() {
        let cases = [
            Felt::ZERO,
            Felt::ONE,
            Felt::THREE,
            Felt::from(-1),
            Felt::from(-2),
            Felt::from(1234567890123456789u64),
            Felt::from_hex_unchecked(
                "0x4d6e41de886ac83938da3456ccf1481182687989ead34d9d35236f0864575a0",
            ),
            Felt::MAX,
        ];
        for &a in &cases {
            for &b in &cases {
                let mut dst = [0u8; 32];
                cairo_native__felt252_mul(&mut dst, &a.to_bytes_le(), &b.to_bytes_le());
                assert_eq!(
                    Felt::from_bytes_le(&dst),
                    a * b,
                    "felt252_mul({a:#x}, {b:#x}) gave the wrong product"
                );
            }
        }
    }

    pub fn felt252_short_str(value: &str) -> Felt {
        let values: Vec<_> = value
            .chars()
            .filter_map(|c| c.is_ascii().then_some(c as u8))
            .collect();

        assert!(values.len() < 32);
        Felt::from_bytes_be_slice(&values)
    }

    #[test]
    fn test_debug_print() {
        let dir = env::temp_dir();
        fs::remove_file(dir.join("print.txt")).ok();
        let mut file = File::create_new(dir.join("print.txt")).unwrap();
        {
            let fd = file.as_raw_fd();
            let data = felt252_short_str("hello world");
            let data = data.to_bytes_le();
            unsafe { cairo_native__libfunc__debug__print(fd, &data, 1) };
        }
        file.seek(std::io::SeekFrom::Start(0)).unwrap();

        let mut result = String::new();
        file.read_to_string(&mut result).unwrap();

        assert_eq!(
            result,
            "[DEBUG]\t0x68656c6c6f20776f726c64 ('hello world')\n"
        );
    }

    #[test]
    fn test_pederesen() {
        let mut dst = [0; 32];
        let lhs = Felt::from(1).to_bytes_le();
        let rhs = Felt::from(3).to_bytes_le();

        unsafe {
            cairo_native__libfunc__pedersen(&mut dst, &lhs, &rhs);
        }

        assert_eq!(
            dst,
            [
                84, 98, 174, 134, 3, 124, 237, 179, 166, 110, 159, 98, 170, 35, 83, 237, 130, 154,
                236, 0, 205, 134, 200, 185, 39, 92, 0, 228, 132, 217, 130, 5
            ]
        )
    }

    #[test]
    fn test_hades_permutation() {
        let mut op0 = Felt::from(1).to_bytes_le();
        let mut op1 = Felt::from(1).to_bytes_le();
        let mut op2 = Felt::from(1).to_bytes_le();

        unsafe {
            cairo_native__libfunc__hades_permutation(&mut op0, &mut op1, &mut op2);
        }

        assert_eq!(
            Felt::from_bytes_le(&op0),
            Felt::from_hex("0x4ebdde1149fcacbb41e4fc342432a48c97994fd045f432ad234ae9279269779")
                .unwrap()
        );
        assert_eq!(
            Felt::from_bytes_le(&op1),
            Felt::from_hex("0x7f4cec57dd08b69414f7de7dffa230fc90fa3993673c422408af05831e0cc98")
                .unwrap()
        );
        assert_eq!(
            Felt::from_bytes_le(&op2),
            Felt::from_hex("0x5b5d00fd09caade43caffe70527fa84d5d9cd51e22c2ce115693ecbb5854d6a")
                .unwrap()
        );
    }

    #[test]
    fn test_dict() {
        let dict =
            unsafe { cairo_native__dict_new(size_of::<u64>() as u64, align_of::<u64>() as u64) };

        let key = Felt::ONE.to_bytes_le();
        let mut ptr = ptr::null_mut::<u64>();

        assert_eq!(
            unsafe { cairo_native__dict_get(dict, &key, (&raw mut ptr).cast()) },
            0,
        );
        assert!(!ptr.is_null());
        unsafe { *ptr = 24 };

        assert_eq!(
            unsafe { cairo_native__dict_get(dict, &key, (&raw mut ptr).cast()) },
            1,
        );
        assert!(!ptr.is_null());
        assert_eq!(unsafe { *ptr }, 24);
        unsafe { *ptr = 42 };

        let mut range_check = 0;
        let mut gas = 0;

        unsafe { cairo_native__dict_squash(dict, &mut range_check, &mut gas) };
        assert_eq!(gas, 4050);

        // Dict dup/drop are noops with arena allocation.
        // Just verify the dict is still accessible after squash.
        assert_eq!(
            unsafe { cairo_native__dict_get(dict, &key, (&raw mut ptr).cast()) },
            1,
        );
        assert!(!ptr.is_null());
        assert_eq!(unsafe { *ptr }, 42);
    }

    #[test]
    fn test_ec__ec_point() {
        let mut point = [Felt::ZERO.to_bytes_le(), Felt::ZERO.to_bytes_le()];
        let result = unsafe { cairo_native__libfunc__ec__ec_point_try_new_nz(&mut point) };
        // The inifinity point isn't valid.
        assert!(!result);
        let point = AffinePoint::generator();
        let mut point = [point.x().to_bytes_le(), point.y().to_bytes_le()];
        let result = unsafe { cairo_native__libfunc__ec__ec_point_try_new_nz(&mut point) };
        assert!(result);
    }

    #[test]
    fn test_ec__ec_state_add__from_zero() {
        let g = AffinePoint::generator();
        let mut state = [[0u8; 32]; 2];
        let point = [g.x().to_bytes_le(), g.y().to_bytes_le()];
        unsafe { cairo_native__libfunc__ec__ec_state_add(&mut state, &point) };
        assert_eq!(state[0], g.x().to_bytes_le());
        assert_eq!(state[1], g.y().to_bytes_le());
    }

    #[test]
    fn test_ec__ec_state_add__to_zero() {
        let g = AffinePoint::generator();
        let neg_g = AffinePoint::new_unchecked(g.x(), -g.y());
        let mut state = [g.x().to_bytes_le(), g.y().to_bytes_le()];
        let point = [neg_g.x().to_bytes_le(), neg_g.y().to_bytes_le()];
        unsafe { cairo_native__libfunc__ec__ec_state_add(&mut state, &point) };
        assert_eq!(state, [[0u8; 32]; 2]);
    }

    #[test]
    fn test_ec__ec_state_add_mul__from_zero() {
        let g = AffinePoint::generator();
        let mut state = [[0u8; 32]; 2];
        let scalar = Felt::ONE.to_bytes_le();
        let point = [g.x().to_bytes_le(), g.y().to_bytes_le()];
        unsafe { cairo_native__libfunc__ec__ec_state_add_mul(&mut state, &scalar, &point) };
        assert_eq!(state[0], g.x().to_bytes_le());
        assert_eq!(state[1], g.y().to_bytes_le());
    }

    #[test]
    fn test_ec__ec_state_add_mul__to_zero() {
        // (-G) + 1*G = identity
        let g = AffinePoint::generator();
        let neg_g = AffinePoint::new_unchecked(g.x(), -g.y());
        let mut state = [neg_g.x().to_bytes_le(), neg_g.y().to_bytes_le()];
        let scalar = Felt::ONE.to_bytes_le();
        let point = [g.x().to_bytes_le(), g.y().to_bytes_le()];
        unsafe { cairo_native__libfunc__ec__ec_state_add_mul(&mut state, &scalar, &point) };
        assert_eq!(state, [[0u8; 32]; 2]);
    }

    #[test]
    fn test_ec__ec_state_finalize__zero() {
        let state = [[0u8; 32]; 2];
        let mut point = [[0u8; 32]; 2];
        let result =
            unsafe { cairo_native__libfunc__ec__ec_state_try_finalize_nz(&mut point, &state) };
        assert!(!result);
    }

    #[test]
    fn test_ec__ec_state_finalize__non_zero() {
        let g = AffinePoint::generator();
        let state = [g.x().to_bytes_le(), g.y().to_bytes_le()];
        let mut point = [[0u8; 32]; 2];
        let result =
            unsafe { cairo_native__libfunc__ec__ec_state_try_finalize_nz(&mut point, &state) };
        assert!(result);
        assert_eq!(point[0], g.x().to_bytes_le());
        assert_eq!(point[1], g.y().to_bytes_le());
    }

    #[test]
    fn test_ec__ec_point_add() {
        // Test values taken from starknet-rs
        let mut state = [
            Felt::from_dec_str(
                "874739451078007766457464989774322083649278607533249481151382481072868806602",
            )
            .unwrap()
            .to_bytes_le(),
            Felt::from_dec_str(
                "152666792071518830868575557812948353041420400780739481342941381225525861407",
            )
            .unwrap()
            .to_bytes_le(),
        ];

        let point = [
            Felt::from_dec_str(
                "874739451078007766457464989774322083649278607533249481151382481072868806602",
            )
            .unwrap()
            .to_bytes_le(),
            Felt::from_dec_str(
                "152666792071518830868575557812948353041420400780739481342941381225525861407",
            )
            .unwrap()
            .to_bytes_le(),
        ];

        unsafe {
            cairo_native__libfunc__ec__ec_state_add(&mut state, &point);
        };

        assert_eq!(
            state[0],
            Felt::from_dec_str(
                "3324833730090626974525872402899302150520188025637965566623476530814354734325",
            )
            .unwrap()
            .to_bytes_le()
        );
        assert_eq!(
            state[1],
            Felt::from_dec_str(
                "3147007486456030910661996439995670279305852583596209647900952752170983517249",
            )
            .unwrap()
            .to_bytes_le()
        );
    }
}
