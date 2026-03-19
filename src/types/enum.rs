//! # Enum type
//!
//! Enumerations are special because they can potentially represent an unlimited amount of things at
//! the same time. They are similar to Rust enums since they can contain data along with the
//! discriminator.
//!
//! ## Layout
//!
//! | Index | Type                 | Description              |
//! | ----- | -------------------- | ------------------------ |
//! |   0   | `iN`                 | Discriminant.            |
//! |   1   | Depends on variants. | Payload.                 |
//!
//! As seen in the table above, an enum's layout is not as simple as concatenating the discriminant
//! with the payload.
//!
//! The discriminant will have the bit width required to store all possible values. The following
//! table contains an example of some number of variants with their discriminant type:
//!
//! | Number of variants | Discriminant type | ABI (in Rust types) |
//! | ------------------ | ----------------- | ------------------- |
//! | 0 or 1             | `i0`              | `()`                |
//! | 2                  | `i1`              | `u8`                |
//! | 3 or 4             | `i2`              | `u8`                |
//! | 5, 6, 7 or 8       | `i3`              | `u8`                |
//! | 9 to 16            | `i4`              | `u8`                |
//! | 129 to 256         | `i8`              | `u8`                |
//! | 257 to 512         | `i9`              | `u16`               |
//! | 32769 to 65536     | `i16`             | `u16`               |
//! | 65537 to 131072    | `i17`             | `u32`               |
//!
//! In Rust, the number of bits and bytes required can be obtained using the following formula,
//! where `n` is the number of bits required, and `m` is the number of bytes required, and
//! `v` is the number of variants.
//!
//! ```math
//! n = { 0         if v == 0
//!     { ⌈log₂(v)⌉ if v != 0
//!
//! m = ⌈n/8⌉
//! ```
//!
//! The payload will then be appended to the discriminant after applying its alignment rules. This
//! will cause unused space between the tag and the payload in most cases.
//!
//! ## Example
//!
//! As an example, the following enum will have the layouts described in the table below.
//!
//! ```cairo
//! enum MyEnum {
//!     U8: u8,
//!     U16: u16,
//!     U32: u32,
//!     U64: u64,
//!     Felt: Felt,
//! }
//! ```
//!
//! ### MyEnum::U8
//!
//! | Index | Type  | ABI (in Rust types) | Alignment | Size | Description   |
//! | ----- | ----- | ------------------- | --------- | ---- | ------------- |
//! |   0   | `i3`  | `u8`                |         1 |    1 | Discriminant. |
//! |   1   | `i8`  | `u8`                |         1 |    1 | Payload.      |
//! |  N/A  | N/A   | `[u8; 38]`          |         1 |   38 | Padding.      |
//!
//! ### MyEnum::U16
//!
//! | Index | Type  | ABI (in Rust types) | Alignment | Size | Description   |
//! | ----- | ----- | ------------------- | --------- | ---- | ------------- |
//! |   0   | `i3`  | `u8`                |         1 |    1 | Discriminant. |
//! |  N/A  | N/A   | `[u8; 1]`           |         1 |    1 | Padding.      |
//! |   1   | `i16` | `u16`               |         2 |    2 | Payload.      |
//! |  N/A  | N/A   | `[u8; 36]`          |         1 |   36 | Padding.      |
//!
//! ### MyEnum::U32
//!
//! | Index | Type  | ABI (in Rust types) | Alignment | Size | Description   |
//! | ----- | ----- | ------------------- | --------- | ---- | ------------- |
//! |   0   | `i3`  | `u8`                |         1 |    1 | Discriminant. |
//! |  N/A  | N/A   | `[u8; 3]`           |         1 |    3 | Padding.      |
//! |   1   | `i32` | `u32`               |         4 |    4 | Payload.      |
//! |  N/A  | N/A   | `[u8; 32]`          |         1 |   32 | Padding.      |
//!
//! ### MyEnum::U64
//!
//! | Index | Type  | ABI (in Rust types) | Alignment | Size | Description   |
//! | ----- | ----- | ------------------- | --------- | ---- | ------------- |
//! |   0   | `i3`  | `u8`                |         1 |    1 | Discriminant. |
//! |  N/A  | N/A   | `[u8; 7]`           |         1 |    7 | Padding.      |
//! |   1   | `i64` | `u64`               |         8 |    8 | Payload.      |
//! |  N/A  | N/A   | `[u8; 24]`          |         1 |   24 | Padding.      |
//!
//! ### MyEnum::Felt
//!
//! | Index | Type   | ABI (in Rust types) | Alignment | Size | Description   |
//! | ----- | ------ | ------------------- | --------- | ---- | ------------- |
//! |   0   | `i3`   | `u8`                |         1 |    1 | Discriminant. |
//! |  N/A  | N/A    | `[u8; 7]`           |         1 |    7 | Padding.      |
//! |   1   | `i252` | `[u64; 4]`          |         8 |   32 | Payload.      |
//!
//! As seen above, while the discriminant is always at the same offset, the payloads don't necessary
//! have the same offset between all variants. It depends on the payload's alignment.
//!
//! In reality, the first variant will have a zero-sized padding between the discriminant and the
//! payload to keep everything consistent and the padding will have its own index, shifting every
//! index below it by one. However all that's been ignored for documenting purposes.
//!
//! An MLIR type cannot be an enumeration (it doesn't exist). We cannot use the type of the largest
//! variant either. This is because the payload of different variants could start at different offsets,
//! so the padding of the largest variant could be part of the payload of a smaller variant. As an
//! optimization, LLVM does not copy the value of the padding, which implies that some data is lost.
//!
//! Instead, we represent enums as an opaque structure that contains an integer and a byte array.
//! The integer is used to ensure that the alignment information is not lost, as we use an integer
//! with the same alignment as the variant with biggest alignment.
//!
//! | Index | Type       | ABI (in Rust types) | Alignment | Size | Description       |
//! | ----- | ---------- | ------------------- | --------- | ---- | ----------------- |
//! |   0   | `i64`      | `u64`               |         8 |    8 | Forces alignment. |
//! |   1   | `[u8; 32]` | `[u8; 32]`          |         1 |   32 | Remaining data.   |
//!
//! This data type can then be reinterpreted safely into any of the concrete variants.

use super::{TypeBuilder, WithSelf};
use crate::{
    error::Result,
    metadata::{
        drop_overrides::DropOverridesMeta, dup_overrides::DupOverridesMeta, MetadataStorage,
    },
    native_panic,
    utils::{get_integer_layout, ProgramRegistryExt},
};
use cairo_lang_sierra::{
    extensions::{
        core::{CoreLibfunc, CoreType},
        enm::EnumConcreteType,
    },
    ids::ConcreteTypeId,
    program_registry::ProgramRegistry,
};
use melior::{
    dialect::{cf, func, llvm},
    helpers::{BuiltinBlockExt, LlvmBlockExt},
    ir::{r#type::IntegerType, Block, BlockLike, Location, Module, Region, Type, Value},
    Context,
};
use std::{
    alloc::Layout,
    collections::{hash_map::Entry, HashMap},
};

/// An MLIR type with its memory layout.
pub type TypeLayout<'ctx> = (Type<'ctx>, Layout);

/// Build the MLIR type.
///
/// Check out [the module](self) for more info.
pub fn build<'ctx>(
    context: &'ctx Context,
    module: &Module<'ctx>,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    metadata: &mut MetadataStorage,
    info: WithSelf<EnumConcreteType>,
) -> Result<Type<'ctx>> {
    // Register enum's clone impl (if required).
    DupOverridesMeta::register_with(
        context,
        module,
        registry,
        metadata,
        info.self_ty(),
        |metadata| {
            let mut needs_override = false;
            for variant in &info.variants {
                registry.build_type(context, module, metadata, variant)?;
                if DupOverridesMeta::is_overriden(metadata, variant) {
                    needs_override = true;
                    break;
                }
            }

            needs_override
                .then(|| build_dup(context, module, registry, metadata, &info))
                .transpose()
        },
    )?;
    DropOverridesMeta::register_with(
        context,
        module,
        registry,
        metadata,
        info.self_ty(),
        |metadata| {
            // The following unwrap is unreachable because `register_with` will always insert it
            // before calling this closure.
            let mut needs_override = false;
            for variant in &info.variants {
                registry.build_type(context, module, metadata, variant)?;

                if DropOverridesMeta::is_overriden(metadata, variant) {
                    needs_override = true;
                    break;
                }
            }

            needs_override
                .then(|| build_drop(context, module, registry, metadata, &info))
                .transpose()
        },
    )?;

    let tag_bits = info.variants.len().next_power_of_two().trailing_zeros();

    let tag_layout = get_integer_layout(tag_bits);
    let layout = info.variants.iter().try_fold(tag_layout, |acc, id| {
        let layout = tag_layout
            .extend(registry.get_type(id)?.layout(registry)?)?
            .0;

        Result::Ok(Layout::from_size_align(
            acc.size().max(layout.size()),
            acc.align().max(layout.align()),
        )?)
    })?;

    let i8_ty = IntegerType::new(context, 8).into();
    Ok(match info.variants.len() {
        0 => llvm::r#type::array(IntegerType::new(context, 8).into(), 0),
        1 => registry.build_type(context, module, metadata, &info.variants[0])?,
        _ if 'block: {
            for type_id in &info.variants {
                if !registry.get_type(type_id)?.is_zst(registry)? {
                    break 'block false;
                }
            }
            true
        } =>
        {
            llvm::r#type::r#struct(
                context,
                &[
                    IntegerType::new(context, tag_bits).into(),
                    llvm::r#type::array(i8_ty, 0),
                ],
                false,
            )
        }
        _ => llvm::r#type::r#struct(
            context,
            &[
                IntegerType::new(context, (8 * layout.align()) as u32).into(),
                llvm::r#type::array(i8_ty, (layout.size() - layout.align()) as u32),
            ],
            false,
        ),
    })
}

fn build_dup<'ctx>(
    context: &'ctx Context,
    module: &Module<'ctx>,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    metadata: &mut MetadataStorage,
    info: &WithSelf<EnumConcreteType>,
) -> Result<Region<'ctx>> {
    let location = Location::unknown(context);

    let self_ty = registry.build_type(context, module, metadata, info.self_ty())?;

    let region = Region::new();
    let entry = region.append_block(Block::new(&[(self_ty, location)]));

    let (layout, (tag_ty, _), variant_tys) = crate::types::r#enum::get_type_for_variants(
        context,
        module,
        registry,
        metadata,
        &info.variants,
    )?;

    match variant_tys.len() {
        0 => native_panic!("attempt to clone a zero-variant enum"),
        1 => {
            let values = DupOverridesMeta::invoke_override(
                context,
                registry,
                module,
                &entry,
                &entry,
                location,
                metadata,
                &info.variants[0],
                entry.arg(0)?,
            )?;

            entry.append_operation(func::r#return(&[values.0, values.1], location));
        }
        _ => {
            let ptr = entry.alloca1(context, location, self_ty, layout.align())?;
            entry.store(context, location, ptr, entry.arg(0)?)?;

            let mut variant_blocks = HashMap::new();
            for (variant_id, variant_ty) in info
                .variants
                .iter()
                .zip(variant_tys.iter().map(|(x, _)| *x))
            {
                if let Entry::Vacant(entry) = variant_blocks.entry(variant_id.id) {
                    let block = entry.insert(region.append_block(Block::new(&[])));

                    let container = block.load(
                        context,
                        location,
                        ptr,
                        llvm::r#type::r#struct(context, &[tag_ty, variant_ty], false),
                    )?;
                    let value = block.extract_value(context, location, container, variant_ty, 1)?;

                    let values = DupOverridesMeta::invoke_override(
                        context, registry, module, block, block, location, metadata, variant_id,
                        value,
                    )?;

                    let value = block.insert_value(context, location, container, values.0, 1)?;
                    block.store(context, location, ptr, value)?;
                    let value0 = block.load(context, location, ptr, self_ty)?;

                    let value = block.insert_value(context, location, container, values.1, 1)?;
                    block.store(context, location, ptr, value)?;
                    let value1 = block.load(context, location, ptr, self_ty)?;

                    block.append_operation(func::r#return(&[value0, value1], location));
                }
            }

            let default_block = region.append_block(Block::new(&[]));

            let tag_value = entry.load(context, location, ptr, tag_ty)?;
            entry.append_operation(cf::switch(
                context,
                &(0..info.variants.len() as _).collect::<Vec<_>>(),
                tag_value,
                tag_ty,
                (&default_block, &[]),
                &info
                    .variants
                    .iter()
                    .map(|id| (&*variant_blocks[&id.id], &[] as &[Value]))
                    .collect::<Vec<_>>(),
                location,
            )?);

            default_block.append_operation(llvm::unreachable(location));
        }
    }

    Ok(region)
}

fn build_drop<'ctx>(
    context: &'ctx Context,
    module: &Module<'ctx>,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    metadata: &mut MetadataStorage,
    info: &WithSelf<EnumConcreteType>,
) -> Result<Region<'ctx>> {
    let location = Location::unknown(context);

    let self_ty = registry.build_type(context, module, metadata, info.self_ty())?;

    let region = Region::new();
    let entry = region.append_block(Block::new(&[(self_ty, location)]));

    let (layout, (tag_ty, _), variant_tys) = crate::types::r#enum::get_type_for_variants(
        context,
        module,
        registry,
        metadata,
        &info.variants,
    )?;

    match variant_tys.len() {
        0 => native_panic!("attempt to drop a zero-variant enum"),
        1 => {
            DropOverridesMeta::invoke_override(
                context,
                registry,
                module,
                &entry,
                &entry,
                location,
                metadata,
                &info.variants[0],
                entry.arg(0)?,
            )?;

            entry.append_operation(func::r#return(&[], location));
        }
        _ => {
            let ptr = entry.alloca1(context, location, self_ty, layout.align())?;
            entry.store(context, location, ptr, entry.arg(0)?)?;

            let mut variant_blocks = HashMap::new();
            for (variant_id, variant_ty) in info
                .variants
                .iter()
                .zip(variant_tys.iter().map(|(x, _)| *x))
            {
                if let Entry::Vacant(entry) = variant_blocks.entry(variant_id.id) {
                    let block = entry.insert(region.append_block(Block::new(&[])));

                    let container = block.load(
                        context,
                        location,
                        ptr,
                        llvm::r#type::r#struct(context, &[tag_ty, variant_ty], false),
                    )?;
                    let value = block.extract_value(context, location, container, variant_ty, 1)?;

                    DropOverridesMeta::invoke_override(
                        context, registry, module, block, block, location, metadata, variant_id,
                        value,
                    )?;

                    block.append_operation(func::r#return(&[], location));
                }
            }

            let default_block = region.append_block(Block::new(&[]));

            let tag_value = entry.load(context, location, ptr, tag_ty)?;
            entry.append_operation(cf::switch(
                context,
                &(0..info.variants.len() as _).collect::<Vec<_>>(),
                tag_value,
                tag_ty,
                (&default_block, &[]),
                &info
                    .variants
                    .iter()
                    .map(|id| (&*variant_blocks[&id.id], &[] as &[Value]))
                    .collect::<Vec<_>>(),
                location,
            )?);

            default_block.append_operation(llvm::unreachable(location));
        }
    }

    Ok(region)
}

/// Extract layout for the default enum representation, its discriminant and all its payloads.
pub fn get_layout_for_variants(
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    variants: &[ConcreteTypeId],
) -> Result<(Layout, Layout, Vec<Layout>)> {
    let tag_bits = variants.len().next_power_of_two().trailing_zeros();
    let tag_layout = get_integer_layout(tag_bits);

    let mut layout = tag_layout;
    let mut output = Vec::with_capacity(variants.len());
    for variant in variants {
        let concrete_payload_ty = registry.get_type(variant)?;
        let payload_layout = concrete_payload_ty.layout(registry)?;

        let full_layout = tag_layout.extend(payload_layout)?.0;
        layout = Layout::from_size_align(
            layout.size().max(full_layout.size()),
            layout.align().max(full_layout.align()),
        )?;

        output.push(payload_layout);
    }

    Ok((layout, tag_layout, output))
}

/// Extract the type and layout for the default enum representation, its discriminant and all its
/// payloads.
// TODO: Change this function to accept a slice of slices (for variants). Not all uses have a slice
// with one `ConcreteTypeId` per variant (deploy_syscalls has two types for the Ok() variant).
// See: https://github.com/starkware-libs/cairo_native/issues/1187/
pub fn get_type_for_variants<'ctx>(
    context: &'ctx Context,
    module: &Module<'ctx>,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    metadata: &mut MetadataStorage,
    variants: &[ConcreteTypeId],
) -> Result<(Layout, TypeLayout<'ctx>, Vec<TypeLayout<'ctx>>)> {
    let tag_bits = variants.len().next_power_of_two().trailing_zeros();
    let tag_layout = get_integer_layout(tag_bits);
    let tag_ty: Type = IntegerType::new(context, tag_bits).into();

    let mut layout = tag_layout;
    let mut output = Vec::with_capacity(variants.len());
    for variant in variants {
        let (payload_ty, payload_layout) =
            registry.build_type_with_layout(context, module, metadata, variant)?;

        let full_layout = tag_layout.extend(payload_layout)?.0;
        layout = Layout::from_size_align(
            layout.size().max(full_layout.size()),
            layout.align().max(full_layout.align()),
        )?;

        output.push((payload_ty, payload_layout));
    }

    Ok((layout, (tag_ty, tag_layout), output))
}

#[cfg(test)]
mod test {
    use crate::{metadata::MetadataStorage, types::TypeBuilder, utils::testing::load_program};
    use cairo_lang_sierra::{
        extensions::core::{CoreLibfunc, CoreType},
        program_registry::ProgramRegistry,
    };
    use melior::{
        ir::{r#type::IntegerType, Location, Module},
        Context,
    };

    #[test]
    fn enum_type_single_variant_no_i0() {
        let program =
            load_program("test_data_artifacts/programs/types/enum_type_single_variant_no_i0");
        let context = Context::new();
        let registry = ProgramRegistry::<CoreType, CoreLibfunc>::new(&program).unwrap();

        let module = Module::new(Location::unknown(&context));
        let mut metadata = MetadataStorage::new();

        let i0_ty = IntegerType::new(&context, 0).into();
        program
            .type_declarations
            .iter()
            .map(|ty| (&ty.id, registry.get_type(&ty.id).unwrap()))
            .map(|(id, ty)| {
                ty.build(&context, &module, &registry, &mut metadata, id)
                    .unwrap()
            })
            .any(|width| width == i0_ty);
    }
}
