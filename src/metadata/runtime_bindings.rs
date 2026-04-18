//! # Runtime library bindings
//!
//! This metadata ensures that the bindings to the runtime functions exist in the current
//! compilation context.

use crate::{
    error::{Error, Result},
    libfuncs::LibfuncHelper,
    utils::get_integer_layout,
};
use cairo_lang_sierra::extensions::qm31::QM31BinaryOperator;
use itertools::Itertools;
use melior::{
    dialect::{
        arith::{self, CmpiPredicate},
        cf, llvm, ods,
    },
    helpers::{ArithBlockExt, BuiltinBlockExt, LlvmBlockExt},
    ir::{
        attribute::{FlatSymbolRefAttribute, StringAttribute, TypeAttribute},
        operation::OperationBuilder,
        r#type::IntegerType,
        Attribute, Block, BlockLike, Identifier, Location, Module, OperationRef, Region, Type,
        Value,
    },
    Context,
};
use std::{
    alloc::Layout,
    collections::HashSet,
    ffi::{c_int, c_void},
};

#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
enum RuntimeBinding {
    Pedersen,
    HadesPermutation,
    EcStateTryFinalizeNz,
    EcStateAddMul,
    EcStateInit,
    EcStateAdd,
    EcPointTryNewNz,
    EcPointFromXNz,
    DictNew,
    DictGet,
    DictSquash,
    DictDrop,
    DictDup,
    GetCostsBuiltin,
    BlakeCompress,
    DebugPrint,
    U31ExtendedEuclideanAlgorithm,
    U252ExtendedEuclideanAlgorithm,
    U384ExtendedEuclideanAlgorithm,
    CircuitArithOperation,
    DictIntoEntries,
    QM31Add,
    QM31Sub,
    QM31Mul,
    QM31Div,
    BoxAlloc,
    #[cfg(feature = "with-cheatcode")]
    VtableCheatcode,
}

impl RuntimeBinding {
    const fn symbol(self) -> &'static str {
        match self {
            RuntimeBinding::DebugPrint => "cairo_native__libfunc__debug__print",
            RuntimeBinding::Pedersen => "cairo_native__libfunc__pedersen",
            RuntimeBinding::HadesPermutation => "cairo_native__libfunc__hades_permutation",
            RuntimeBinding::EcStateTryFinalizeNz => {
                "cairo_native__libfunc__ec__ec_state_try_finalize_nz"
            }
            RuntimeBinding::EcStateAddMul => "cairo_native__libfunc__ec__ec_state_add_mul",
            RuntimeBinding::EcStateInit => "cairo_native__libfunc__ec__ec_state_init",
            RuntimeBinding::EcStateAdd => "cairo_native__libfunc__ec__ec_state_add",
            RuntimeBinding::EcPointTryNewNz => "cairo_native__libfunc__ec__ec_point_try_new_nz",
            RuntimeBinding::EcPointFromXNz => "cairo_native__libfunc__ec__ec_point_from_x_nz",
            RuntimeBinding::DictNew => "cairo_native__dict_new",
            RuntimeBinding::DictGet => "cairo_native__dict_get",
            RuntimeBinding::DictSquash => "cairo_native__dict_squash",
            RuntimeBinding::DictDrop => "cairo_native__dict_drop",
            RuntimeBinding::DictDup => "cairo_native__dict_dup",
            RuntimeBinding::GetCostsBuiltin => "cairo_native__get_costs_builtin",
            RuntimeBinding::BlakeCompress => "cairo_native__libfunc__blake_compress",
            RuntimeBinding::U31ExtendedEuclideanAlgorithm => {
                "cairo_native__u31_extended_euclidean_algorithm"
            }
            RuntimeBinding::U252ExtendedEuclideanAlgorithm => {
                "cairo_native__u252_extended_euclidean_algorithm"
            }
            RuntimeBinding::U384ExtendedEuclideanAlgorithm => {
                "cairo_native__u384_extended_euclidean_algorithm"
            }
            RuntimeBinding::CircuitArithOperation => "cairo_native__circuit_arith_operation",
            RuntimeBinding::DictIntoEntries => "cairo_native__dict_into_entries",
            RuntimeBinding::QM31Add => "cairo_native__libfunc__qm31__qm31_add",
            RuntimeBinding::QM31Sub => "cairo_native__libfunc__qm31__qm31_sub",
            RuntimeBinding::QM31Mul => "cairo_native__libfunc__qm31__qm31_mul",
            RuntimeBinding::QM31Div => "cairo_native__libfunc__qm31__qm31_div",
            RuntimeBinding::BoxAlloc => "cairo_native__box_alloc",
            #[cfg(feature = "with-cheatcode")]
            RuntimeBinding::VtableCheatcode => "cairo_native__vtable_cheatcode",
        }
    }

    /// Returns an `Option` with a function pointer depending on how the binding is implemented.
    ///
    /// - For external bindings (implemented in Rust), it returns `Some`, containing
    ///   a pointer to the corresponding Rust function
    /// - For internal bindings (implemented in MLIR), it returns `None`, since those
    ///   functions are defined within MLIR and invoked by name
    const fn function_ptr(self) -> Option<*const ()> {
        let function_ptr = match self {
            RuntimeBinding::DebugPrint => {
                crate::runtime::cairo_native__libfunc__debug__print as *const ()
            }
            RuntimeBinding::Pedersen => {
                crate::runtime::cairo_native__libfunc__pedersen as *const ()
            }
            RuntimeBinding::HadesPermutation => {
                crate::runtime::cairo_native__libfunc__hades_permutation as *const ()
            }
            RuntimeBinding::EcStateTryFinalizeNz => {
                crate::runtime::cairo_native__libfunc__ec__ec_state_try_finalize_nz as *const ()
            }
            RuntimeBinding::EcStateAddMul => {
                crate::runtime::cairo_native__libfunc__ec__ec_state_add_mul as *const ()
            }
            RuntimeBinding::EcStateInit => {
                crate::runtime::cairo_native__libfunc__ec__ec_state_init as *const ()
            }
            RuntimeBinding::EcStateAdd => {
                crate::runtime::cairo_native__libfunc__ec__ec_state_add as *const ()
            }
            RuntimeBinding::EcPointTryNewNz => {
                crate::runtime::cairo_native__libfunc__ec__ec_point_try_new_nz as *const ()
            }
            RuntimeBinding::EcPointFromXNz => {
                crate::runtime::cairo_native__libfunc__ec__ec_point_from_x_nz as *const ()
            }
            RuntimeBinding::DictNew => crate::runtime::cairo_native__dict_new as *const (),
            RuntimeBinding::DictGet => crate::runtime::cairo_native__dict_get as *const (),
            RuntimeBinding::DictSquash => crate::runtime::cairo_native__dict_squash as *const (),
            RuntimeBinding::DictDrop => crate::runtime::cairo_native__dict_drop as *const (),
            RuntimeBinding::DictDup => crate::runtime::cairo_native__dict_dup as *const (),
            RuntimeBinding::GetCostsBuiltin => {
                crate::runtime::cairo_native__get_costs_builtin as *const ()
            }
            RuntimeBinding::DictIntoEntries => {
                crate::runtime::cairo_native__dict_into_entries as *const ()
            }
            RuntimeBinding::QM31Add => {
                crate::runtime::cairo_native__libfunc__qm31__qm31_add as *const ()
            }
            RuntimeBinding::QM31Sub => {
                crate::runtime::cairo_native__libfunc__qm31__qm31_sub as *const ()
            }
            RuntimeBinding::QM31Mul => {
                crate::runtime::cairo_native__libfunc__qm31__qm31_mul as *const ()
            }
            RuntimeBinding::QM31Div => {
                crate::runtime::cairo_native__libfunc__qm31__qm31_div as *const ()
            }
            RuntimeBinding::BlakeCompress => {
                crate::runtime::cairo_native__libfunc__blake_compress as *const ()
            }
            RuntimeBinding::U31ExtendedEuclideanAlgorithm
            | RuntimeBinding::U252ExtendedEuclideanAlgorithm
            | RuntimeBinding::U384ExtendedEuclideanAlgorithm => return None,
            RuntimeBinding::CircuitArithOperation => return None,
            RuntimeBinding::BoxAlloc => crate::runtime::cairo_native__box_alloc as *const (),
            #[cfg(feature = "with-cheatcode")]
            RuntimeBinding::VtableCheatcode => {
                crate::starknet::cairo_native__vtable_cheatcode as *const ()
            }
        };
        Some(function_ptr)
    }
}

// This enum is used when performing circuit arithmetic operations.
// Inversion is not included because it is handled separately.
#[repr(u8)]
#[derive(Clone, Copy)]
pub enum CircuitArithOperationType {
    Add,
    Sub,
    Mul,
}

/// Runtime library bindings metadata.
#[derive(Debug, Default)]
pub struct RuntimeBindingsMeta {
    active_map: HashSet<RuntimeBinding>,
}

impl RuntimeBindingsMeta {
    /// Register the global for the given binding, if not yet registered, and return
    /// a pointer to the stored function.
    ///
    /// For the function to be available, `setup_runtime` must be called before running the module
    fn build_function<'c, 'a>(
        &mut self,
        context: &'c Context,
        module: &Module,
        block: &'a Block<'c>,
        location: Location<'c>,
        binding: RuntimeBinding,
    ) -> Result<Value<'c, 'a>> {
        if self.active_map.insert(binding) {
            module.body().append_operation(
                ods::llvm::mlir_global(
                    context,
                    Region::new(),
                    TypeAttribute::new(llvm::r#type::pointer(context, 0)),
                    StringAttribute::new(context, binding.symbol()),
                    Attribute::parse(context, "#llvm.linkage<weak>")
                        .ok_or(Error::ParseAttributeError)?,
                    location,
                )
                .into(),
            );
        }

        let global_address = block.append_op_result(
            ods::llvm::mlir_addressof(
                context,
                llvm::r#type::pointer(context, 0),
                FlatSymbolRefAttribute::new(context, binding.symbol()),
                location,
            )
            .into(),
        )?;

        Ok(block.load(
            context,
            location,
            global_address,
            llvm::r#type::pointer(context, 0),
        )?)
    }

    /// Build if necessary the extended euclidean algorithm used in circuit inverse gates.
    ///
    /// After checking, calls the MLIR function with arguments `a` and `b` which are the initial remainders
    /// used in the algorithm and returns a `Value` containing a struct where the first element is the
    /// greatest common divisor of `a` and `b` and the second element is the bezout coefficient x.
    ///
    /// This implementation is only for felt252, which uses u31 integers.
    pub fn u31_extended_euclidean_algorithm<'c, 'a>(
        &mut self,
        context: &'c Context,
        module: &Module,
        block: &'a Block<'c>,
        location: Location<'c>,
        a: Value<'c, '_>,
        b: Value<'c, '_>,
    ) -> Result<Value<'c, 'a>>
    where
        'c: 'a,
    {
        let integer_type = IntegerType::new(context, 31).into();
        let func_symbol = RuntimeBinding::U31ExtendedEuclideanAlgorithm.symbol();
        if self
            .active_map
            .insert(RuntimeBinding::U31ExtendedEuclideanAlgorithm)
        {
            build_egcd_function(module, context, location, func_symbol, integer_type)?;
        }
        // The struct returned by the function that contains both of the results
        let return_type = llvm::r#type::r#struct(context, &[integer_type, integer_type], false);
        Ok(block
            .append_operation(
                OperationBuilder::new("llvm.call", location)
                    .add_attributes(&[(
                        Identifier::new(context, "callee"),
                        FlatSymbolRefAttribute::new(context, func_symbol).into(),
                    )])
                    .add_operands(&[a, b])
                    .add_results(&[return_type])
                    .build()?,
            )
            .result(0)?
            .into())
    }

    /// Build if necessary the extended euclidean algorithm used in circuit inverse gates.
    ///
    /// After checking, calls the MLIR function with arguments `a` and `b` which are the initial remainders
    /// used in the algorithm and returns a `Value` containing a struct where the first element is the
    /// greatest common divisor of `a` and `b` and the second element is the bezout coefficient x.
    ///
    /// This implementation is only for felt252, which uses u252 integers.
    pub fn u252_extended_euclidean_algorithm<'c, 'a>(
        &mut self,
        context: &'c Context,
        module: &Module,
        block: &'a Block<'c>,
        location: Location<'c>,
        a: Value<'c, '_>,
        b: Value<'c, '_>,
    ) -> Result<Value<'c, 'a>>
    where
        'c: 'a,
    {
        let integer_type = IntegerType::new(context, 252).into();
        let func_symbol = RuntimeBinding::U252ExtendedEuclideanAlgorithm.symbol();
        if self
            .active_map
            .insert(RuntimeBinding::U252ExtendedEuclideanAlgorithm)
        {
            build_egcd_function(module, context, location, func_symbol, integer_type)?;
        }
        // The struct returned by the function that contains both of the results
        let return_type = llvm::r#type::r#struct(context, &[integer_type, integer_type], false);
        Ok(block
            .append_operation(
                OperationBuilder::new("llvm.call", location)
                    .add_attributes(&[(
                        Identifier::new(context, "callee"),
                        FlatSymbolRefAttribute::new(context, func_symbol).into(),
                    )])
                    .add_operands(&[a, b])
                    .add_results(&[return_type])
                    .build()?,
            )
            .result(0)?
            .into())
    }

    /// Similar to [felt252_extended_euclidean_algorithm](Self::felt252_extended_euclidean_algorithm).
    ///
    /// The difference with the other is that this function is meant to be used
    /// with circuits, which use u384 integers.
    pub fn u384_extended_euclidean_algorithm<'c, 'a>(
        &mut self,
        context: &'c Context,
        module: &Module,
        block: &'a Block<'c>,
        location: Location<'c>,
        a: Value<'c, '_>,
        b: Value<'c, '_>,
    ) -> Result<Value<'c, 'a>>
    where
        'c: 'a,
    {
        let integer_type = IntegerType::new(context, 384).into();
        let func_symbol = RuntimeBinding::U384ExtendedEuclideanAlgorithm.symbol();
        if self
            .active_map
            .insert(RuntimeBinding::U384ExtendedEuclideanAlgorithm)
        {
            build_egcd_function(module, context, location, func_symbol, integer_type)?;
        }
        // The struct returned by the function that contains both of the results
        let return_type = llvm::r#type::r#struct(context, &[integer_type, integer_type], false);
        Ok(block
            .append_operation(
                OperationBuilder::new("llvm.call", location)
                    .add_attributes(&[(
                        Identifier::new(context, "callee"),
                        FlatSymbolRefAttribute::new(context, func_symbol).into(),
                    )])
                    .add_operands(&[a, b])
                    .add_results(&[return_type])
                    .build()?,
            )
            .result(0)?
            .into())
    }

    /// Builds, if necessary, the circuit operation function, used to perform
    /// circuit arithmetic operations.
    ///
    /// ## Operands
    /// - `op`: an enum telling which arithmetic operation to perform.
    /// - `lhs_value`: u384 operand.
    /// - `rhs_value`: u384 operand.
    /// - `circuit_modulus`: u384 circuit modulus.
    ///
    /// This function only handles addition, substraction and multiplication
    /// operations. The inversion operation was excluded as it is already handled
    /// by the [`extended_euclidean_algorithm`]
    #[allow(clippy::too_many_arguments)]
    pub fn circuit_arith_operation<'c, 'a>(
        &mut self,
        context: &'c Context,
        module: &Module,
        block: &'a Block<'c>,
        location: Location<'c>,
        op_type: CircuitArithOperationType,
        lhs_value: Value<'c, '_>,
        rhs_value: Value<'c, '_>,
        circuit_modulus: Value<'c, '_>,
    ) -> Result<Value<'c, 'a>>
    where
        'c: 'a,
    {
        let func_symbol = RuntimeBinding::CircuitArithOperation.symbol();
        if self
            .active_map
            .insert(RuntimeBinding::CircuitArithOperation)
        {
            build_circuit_arith_operation(context, module, location, func_symbol)?;
        }

        let op_tag = block.const_int(context, location, op_type as u8, 2)?;
        let return_type = IntegerType::new(context, 384).into();

        Ok(block.append_op_result(
            OperationBuilder::new("llvm.call", location)
                .add_attributes(&[(
                    Identifier::new(context, "callee"),
                    FlatSymbolRefAttribute::new(context, func_symbol).into(),
                )])
                .add_operands(&[op_tag, lhs_value, rhs_value, circuit_modulus])
                .add_results(&[return_type])
                .build()?,
        )?)
    }

    /// Register if necessary, then invoke the `debug::print()` function.
    #[allow(clippy::too_many_arguments)]
    pub fn libfunc_debug_print<'c, 'a>(
        &mut self,
        context: &'c Context,
        module: &Module,
        block: &'a Block<'c>,
        target_fd: Value<'c, '_>,
        values_ptr: Value<'c, '_>,
        values_len: Value<'c, '_>,
        location: Location<'c>,
    ) -> Result<Value<'c, 'a>>
    where
        'c: 'a,
    {
        let function =
            self.build_function(context, module, block, location, RuntimeBinding::DebugPrint)?;

        Ok(block
            .append_operation(
                OperationBuilder::new("llvm.call", location)
                    .add_operands(&[function])
                    .add_operands(&[target_fd, values_ptr, values_len])
                    .add_results(&[IntegerType::new(context, 32).into()])
                    .build()?,
            )
            .result(0)?
            .into())
    }

    /// Register if necessary, then invoke the `pedersen()` function.
    #[allow(clippy::too_many_arguments)]
    pub fn libfunc_pedersen<'c, 'a>(
        &mut self,
        context: &'c Context,
        module: &Module,
        block: &'a Block<'c>,
        dst_ptr: Value<'c, '_>,
        lhs_ptr: Value<'c, '_>,
        rhs_ptr: Value<'c, '_>,
        location: Location<'c>,
    ) -> Result<OperationRef<'c, 'a>>
    where
        'c: 'a,
    {
        let function =
            self.build_function(context, module, block, location, RuntimeBinding::Pedersen)?;

        Ok(block.append_operation(
            OperationBuilder::new("llvm.call", location)
                .add_operands(&[function])
                .add_operands(&[dst_ptr, lhs_ptr, rhs_ptr])
                .build()?,
        ))
    }

    /// Register if necessary, then invoke the `poseidon()` function.
    /// The passed pointers serve both as in/out pointers. I.E results are stored in the given pointers.
    #[allow(clippy::too_many_arguments)]
    pub fn libfunc_hades_permutation<'c, 'a>(
        &mut self,
        context: &'c Context,
        module: &Module,
        block: &'a Block<'c>,
        op0_ptr: Value<'c, '_>,
        op1_ptr: Value<'c, '_>,
        op2_ptr: Value<'c, '_>,
        location: Location<'c>,
    ) -> Result<OperationRef<'c, 'a>>
    where
        'c: 'a,
    {
        let function = self.build_function(
            context,
            module,
            block,
            location,
            RuntimeBinding::HadesPermutation,
        )?;

        Ok(block.append_operation(
            OperationBuilder::new("llvm.call", location)
                .add_operands(&[function])
                .add_operands(&[op0_ptr, op1_ptr, op2_ptr])
                .build()?,
        ))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn libfunc_blake_compress<'c, 'a>(
        &mut self,
        context: &'c Context,
        module: &Module,
        block: &'a Block<'c>,
        state: Value<'c, 'a>,
        message: Value<'c, 'a>,
        count_bytes: Value<'c, 'a>,
        finalize: Value<'c, 'a>,
        location: Location<'c>,
    ) -> Result<OperationRef<'c, 'a>>
    where
        'c: 'a,
    {
        let function = self.build_function(
            context,
            module,
            block,
            location,
            RuntimeBinding::BlakeCompress,
        )?;

        Ok(block.append_operation(
            OperationBuilder::new("llvm.call", location)
                .add_operands(&[function])
                .add_operands(&[state, message, count_bytes, finalize])
                .build()?,
        ))
    }

    /// Register if necessary, then invoke the `ec_point_from_x_nz()` function.
    pub fn libfunc_ec_point_from_x_nz<'c, 'a>(
        &mut self,
        context: &'c Context,
        module: &Module,
        block: &'a Block<'c>,
        point_ptr: Value<'c, '_>,
        location: Location<'c>,
    ) -> Result<OperationRef<'c, 'a>>
    where
        'c: 'a,
    {
        let function = self.build_function(
            context,
            module,
            block,
            location,
            RuntimeBinding::EcPointFromXNz,
        )?;

        Ok(block.append_operation(
            OperationBuilder::new("llvm.call", location)
                .add_operands(&[function])
                .add_operands(&[point_ptr])
                .add_results(&[IntegerType::new(context, 1).into()])
                .build()?,
        ))
    }

    /// Register if necessary, then invoke the `ec_point_try_new_nz()` function.
    pub fn libfunc_ec_point_try_new_nz<'c, 'a>(
        &mut self,
        context: &'c Context,
        module: &Module,
        block: &'a Block<'c>,
        point_ptr: Value<'c, '_>,
        location: Location<'c>,
    ) -> Result<OperationRef<'c, 'a>>
    where
        'c: 'a,
    {
        let function = self.build_function(
            context,
            module,
            block,
            location,
            RuntimeBinding::EcPointTryNewNz,
        )?;

        Ok(block.append_operation(
            OperationBuilder::new("llvm.call", location)
                .add_operands(&[function])
                .add_operands(&[point_ptr])
                .add_results(&[IntegerType::new(context, 1).into()])
                .build()?,
        ))
    }

    /// Register if necessary, then invoke the `ec_state_init()` function.
    pub fn libfunc_ec_state_init<'c, 'a>(
        &mut self,
        context: &'c Context,
        module: &Module,
        block: &'a Block<'c>,
        state_ptr: Value<'c, '_>,
        location: Location<'c>,
    ) -> Result<OperationRef<'c, 'a>>
    where
        'c: 'a,
    {
        let function = self.build_function(
            context,
            module,
            block,
            location,
            RuntimeBinding::EcStateInit,
        )?;

        Ok(block.append_operation(
            OperationBuilder::new("llvm.call", location)
                .add_operands(&[function])
                .add_operands(&[state_ptr])
                .build()?,
        ))
    }

    /// Register if necessary, then invoke the `ec_state_add()` function.
    pub fn libfunc_ec_state_add<'c, 'a>(
        &mut self,
        context: &'c Context,
        module: &Module,
        block: &'a Block<'c>,
        state_ptr: Value<'c, '_>,
        point_ptr: Value<'c, '_>,
        location: Location<'c>,
    ) -> Result<OperationRef<'c, 'a>>
    where
        'c: 'a,
    {
        let function =
            self.build_function(context, module, block, location, RuntimeBinding::EcStateAdd)?;

        Ok(block.append_operation(
            OperationBuilder::new("llvm.call", location)
                .add_operands(&[function])
                .add_operands(&[state_ptr, point_ptr])
                .build()?,
        ))
    }

    /// Register if necessary, then invoke the `ec_state_add_mul()` function.
    #[allow(clippy::too_many_arguments)]
    pub fn libfunc_ec_state_add_mul<'c, 'a>(
        &mut self,
        context: &'c Context,
        module: &Module,
        block: &'a Block<'c>,
        state_ptr: Value<'c, '_>,
        scalar_ptr: Value<'c, '_>,
        point_ptr: Value<'c, '_>,
        location: Location<'c>,
    ) -> Result<OperationRef<'c, 'a>>
    where
        'c: 'a,
    {
        let function = self.build_function(
            context,
            module,
            block,
            location,
            RuntimeBinding::EcStateAddMul,
        )?;

        Ok(block.append_operation(
            OperationBuilder::new("llvm.call", location)
                .add_operands(&[function])
                .add_operands(&[state_ptr, scalar_ptr, point_ptr])
                .build()?,
        ))
    }

    pub fn libfunc_ec_state_try_finalize_nz<'c, 'a>(
        &mut self,
        context: &'c Context,
        module: &Module,
        block: &'a Block<'c>,
        point_ptr: Value<'c, '_>,
        state_ptr: Value<'c, '_>,
        location: Location<'c>,
    ) -> Result<OperationRef<'c, 'a>>
    where
        'c: 'a,
    {
        let function = self.build_function(
            context,
            module,
            block,
            location,
            RuntimeBinding::EcStateTryFinalizeNz,
        )?;

        Ok(block.append_operation(
            OperationBuilder::new("llvm.call", location)
                .add_operands(&[function])
                .add_operands(&[point_ptr, state_ptr])
                .add_results(&[IntegerType::new(context, 1).into()])
                .build()?,
        ))
    }

    /// Register QM31 binary operation function if necessary and invoke it.
    /// The operation depends on the `op` argument which could indicate:
    /// - Add operation
    /// - Sub operation
    /// - Mul operation
    /// - Div operation
    ///
    /// Executes the operation on the `QM31` values referenced by `lhs_ptr` and `rhs_ptr`,
    /// and returns the resulting `QM31`.
    #[allow(clippy::too_many_arguments)]
    pub fn libfunc_qm31_bin_op<'c, 'a>(
        &mut self,
        context: &'c Context,
        module: &Module,
        block: &'a Block<'c>,
        lhs_ptr: Value<'c, '_>,
        rhs_ptr: Value<'c, '_>,
        op: QM31BinaryOperator,
        location: Location<'c>,
    ) -> Result<Value<'c, 'a>>
    where
        'c: 'a,
    {
        let qm31_ty = llvm::r#type::array(IntegerType::new(context, 31).into(), 4);
        let res_ptr = block.alloca1(context, location, qm31_ty, get_integer_layout(31).align())?;

        let function = match op {
            QM31BinaryOperator::Add => {
                self.build_function(context, module, block, location, RuntimeBinding::QM31Add)?
            }
            QM31BinaryOperator::Sub => {
                self.build_function(context, module, block, location, RuntimeBinding::QM31Sub)?
            }
            QM31BinaryOperator::Mul => {
                self.build_function(context, module, block, location, RuntimeBinding::QM31Mul)?
            }
            QM31BinaryOperator::Div => {
                self.build_function(context, module, block, location, RuntimeBinding::QM31Div)?
            }
        };

        block.append_operation(
            OperationBuilder::new("llvm.call", location)
                .add_operands(&[function])
                .add_operands(&[lhs_ptr, rhs_ptr, res_ptr])
                .build()?,
        );

        Ok(block.load(context, location, res_ptr, qm31_ty)?)
    }

    /// Register the `cairo_native__box_alloc` global if necessary, then invoke
    /// it to allocate `size` bytes with alignment `align` from the per-invocation
    /// arena.  Returns an opaque pointer to the allocated memory.
    pub fn box_alloc<'c, 'a>(
        &mut self,
        context: &'c Context,
        module: &Module,
        block: &'a Block<'c>,
        location: Location<'c>,
        size: Value<'c, 'a>,
        align: Value<'c, 'a>,
    ) -> Result<Value<'c, 'a>>
    where
        'c: 'a,
    {
        let function =
            self.build_function(context, module, block, location, RuntimeBinding::BoxAlloc)?;

        Ok(block.append_op_result(
            OperationBuilder::new("llvm.call", location)
                .add_operands(&[function])
                .add_operands(&[size, align])
                .add_results(&[llvm::r#type::pointer(context, 0)])
                .build()?,
        )?)
    }

    /// Register if necessary, then invoke the `dict_alloc_new()` function.
    ///
    /// Returns a opaque pointer as the result.
    #[allow(clippy::too_many_arguments)]
    pub fn dict_new<'c, 'a>(
        &mut self,
        context: &'c Context,
        module: &Module,
        block: &'a Block<'c>,
        location: Location<'c>,
        drop_fn: Option<Value<'c, 'a>>,
        layout: Layout,
    ) -> Result<Value<'c, 'a>>
    where
        'c: 'a,
    {
        let function =
            self.build_function(context, module, block, location, RuntimeBinding::DictNew)?;

        let i64_ty = IntegerType::new(context, 64).into();
        let size = block.const_int_from_type(context, location, layout.size(), i64_ty)?;
        let align = block.const_int_from_type(context, location, layout.align(), i64_ty)?;

        let drop_fn = match drop_fn {
            Some(x) => x,
            None => {
                block.append_op_result(llvm::zero(llvm::r#type::pointer(context, 0), location))?
            }
        };

        Ok(block.append_op_result(
            OperationBuilder::new("llvm.call", location)
                .add_operands(&[function])
                .add_operands(&[size, align, drop_fn])
                .add_results(&[llvm::r#type::pointer(context, 0)])
                .build()?,
        )?)
    }

    /// Register if necessary, then invoke the `dict_alloc_new()` function.
    ///
    /// Returns a opaque pointer as the result.
    #[allow(clippy::too_many_arguments)]
    pub fn dict_drop<'c, 'a>(
        &mut self,
        context: &'c Context,
        module: &Module,
        block: &'a Block<'c>,
        ptr: Value<'c, 'a>,
        location: Location<'c>,
    ) -> Result<OperationRef<'c, 'a>>
    where
        'c: 'a,
    {
        let function =
            self.build_function(context, module, block, location, RuntimeBinding::DictDrop)?;

        Ok(block.append_operation(
            OperationBuilder::new("llvm.call", location)
                .add_operands(&[function])
                .add_operands(&[ptr])
                .build()?,
        ))
    }

    /// Register if necessary, then invoke the `dict_alloc_new()` function.
    ///
    /// Returns a opaque pointer as the result.
    #[allow(clippy::too_many_arguments)]
    pub fn dict_dup<'c, 'a>(
        &mut self,
        context: &'c Context,
        module: &Module,
        block: &'a Block<'c>,
        ptr: Value<'c, 'a>,
        location: Location<'c>,
    ) -> Result<Value<'c, 'a>>
    where
        'c: 'a,
    {
        let function =
            self.build_function(context, module, block, location, RuntimeBinding::DictDup)?;

        Ok(block.append_op_result(
            OperationBuilder::new("llvm.call", location)
                .add_operands(&[function])
                .add_operands(&[ptr])
                .add_results(&[llvm::r#type::pointer(context, 0)])
                .build()?,
        )?)
    }

    /// Register if necessary, then invoke the `dict_get()` function.
    ///
    /// Gets the value for a given key, the returned pointer is null if not found.
    ///
    /// Returns a opaque pointer as the result.
    #[allow(clippy::too_many_arguments)]
    pub fn dict_get<'c, 'a>(
        &mut self,
        context: &'c Context,
        helper: &LibfuncHelper<'c, 'a>,
        block: &'a Block<'c>,
        dict_ptr: Value<'c, 'a>, // ptr to the dict
        key_ptr: Value<'c, 'a>,  // key must be a ptr to Felt
        location: Location<'c>,
    ) -> Result<(Value<'c, 'a>, Value<'c, 'a>)>
    where
        'c: 'a,
    {
        let function =
            self.build_function(context, helper, block, location, RuntimeBinding::DictGet)?;

        let value_ptr = helper.init_block().alloca1(
            context,
            location,
            llvm::r#type::pointer(context, 0),
            align_of::<*mut ()>(),
        )?;

        let is_present = block.append_op_result(
            OperationBuilder::new("llvm.call", location)
                .add_operands(&[function])
                .add_operands(&[dict_ptr, key_ptr, value_ptr])
                .add_results(&[IntegerType::new(context, c_int::BITS).into()])
                .build()?,
        )?;

        let value_ptr = block.load(
            context,
            location,
            value_ptr,
            llvm::r#type::pointer(context, 0),
        )?;

        Ok((is_present, value_ptr))
    }

    /// Register if necessary, then invoke the `dict_gas_refund()` function.
    ///
    /// Compute the total gas refund for the dictionary.
    ///
    /// Returns a u64 of the result.
    #[allow(clippy::too_many_arguments)]
    pub fn dict_squash<'c, 'a>(
        &mut self,
        context: &'c Context,
        module: &Module,
        block: &'a Block<'c>,
        dict_ptr: Value<'c, 'a>,        // ptr to the dict
        range_check_ptr: Value<'c, 'a>, // ptr to range check
        gas_ptr: Value<'c, 'a>,         // ptr to gas
        location: Location<'c>,
    ) -> Result<OperationRef<'c, 'a>>
    where
        'c: 'a,
    {
        let function =
            self.build_function(context, module, block, location, RuntimeBinding::DictSquash)?;

        Ok(block.append_operation(
            OperationBuilder::new("llvm.call", location)
                .add_operands(&[function])
                .add_operands(&[dict_ptr, range_check_ptr, gas_ptr])
                .add_results(&[IntegerType::new(context, 64).into()])
                .build()?,
        ))
    }

    /// Register if necessary, then invoke the `dict_into_entries()` function.
    ///
    /// Returns an array with the tuples of the form (felt252, T, T) by storing it
    /// on `array_ptr`.
    #[allow(clippy::too_many_arguments)]
    pub fn dict_into_entries<'c, 'a>(
        &mut self,
        context: &'c Context,
        helper: &LibfuncHelper<'c, 'a>,
        block: &'a Block<'c>,
        dict_ptr: Value<'c, 'a>,
        array_ptr: Value<'c, 'a>,
        location: Location<'c>,
    ) -> Result<OperationRef<'c, 'a>>
    where
        'c: 'a,
    {
        let function = self.build_function(
            context,
            helper,
            block,
            location,
            RuntimeBinding::DictIntoEntries,
        )?;

        Ok(block.append_operation(
            OperationBuilder::new("llvm.call", location)
                .add_operands(&[function])
                .add_operands(&[dict_ptr, array_ptr])
                .build()?,
        ))
    }

    // Register if necessary, then invoke the `get_costs_builtin()` function.
    #[allow(clippy::too_many_arguments)]
    pub fn get_costs_builtin<'c, 'a>(
        &mut self,
        context: &'c Context,
        module: &Module,
        block: &'a Block<'c>,
        location: Location<'c>,
    ) -> Result<OperationRef<'c, 'a>>
    where
        'c: 'a,
    {
        let function = self.build_function(
            context,
            module,
            block,
            location,
            RuntimeBinding::GetCostsBuiltin,
        )?;

        Ok(block.append_operation(
            OperationBuilder::new("llvm.call", location)
                .add_operands(&[function])
                .add_results(&[llvm::r#type::pointer(context, 0)])
                .build()?,
        ))
    }

    /// Register if necessary, then invoke the `vtable_cheatcode()` runtime function.
    ///
    /// Calls the cheatcode syscall with the given arguments.
    ///
    /// The result is stored in `result_ptr`.
    #[allow(clippy::too_many_arguments)]
    #[cfg(feature = "with-cheatcode")]
    pub fn vtable_cheatcode<'c, 'a>(
        &mut self,
        context: &'c Context,
        module: &Module,
        block: &'a Block<'c>,
        location: Location<'c>,
        result_ptr: Value<'c, 'a>,
        selector_ptr: Value<'c, 'a>,
        args: Value<'c, 'a>,
    ) -> Result<OperationRef<'c, 'a>>
    where
        'c: 'a,
    {
        let function = self.build_function(
            context,
            module,
            block,
            location,
            RuntimeBinding::VtableCheatcode,
        )?;

        Ok(block.append_operation(
            OperationBuilder::new("llvm.call", location)
                .add_operands(&[function])
                .add_operands(&[result_ptr, selector_ptr, args])
                .build()?,
        ))
    }
}

pub fn setup_runtime(find_symbol_ptr: impl Fn(&str) -> Option<*mut c_void>) {
    for binding in [
        RuntimeBinding::DebugPrint,
        RuntimeBinding::Pedersen,
        RuntimeBinding::HadesPermutation,
        RuntimeBinding::EcStateTryFinalizeNz,
        RuntimeBinding::EcStateAddMul,
        RuntimeBinding::EcStateInit,
        RuntimeBinding::EcStateAdd,
        RuntimeBinding::EcPointTryNewNz,
        RuntimeBinding::EcPointFromXNz,
        RuntimeBinding::DictNew,
        RuntimeBinding::DictGet,
        RuntimeBinding::DictSquash,
        RuntimeBinding::DictDrop,
        RuntimeBinding::DictDup,
        RuntimeBinding::GetCostsBuiltin,
        RuntimeBinding::BlakeCompress,
        RuntimeBinding::DebugPrint,
        RuntimeBinding::DictIntoEntries,
        RuntimeBinding::QM31Add,
        RuntimeBinding::QM31Sub,
        RuntimeBinding::QM31Mul,
        RuntimeBinding::QM31Div,
        RuntimeBinding::BoxAlloc,
        #[cfg(feature = "with-cheatcode")]
        RuntimeBinding::VtableCheatcode,
    ] {
        if let Some(global) = find_symbol_ptr(binding.symbol()) {
            let global = global.cast::<*const ()>();
            unsafe {
                if let Some(function_ptr) = binding.function_ptr() {
                    *global = function_ptr;
                };
            }
        }
    }
}

/// Build the extended euclidean algorithm MLIR function.
///
/// The extended euclidean algorithm calculates the greatest common divisor
/// (gcd) of two integers `a` and `b`, as well as the Bézout coefficients `x`
/// and `y` such that `ax + by = gcd(a,b)`. If `gcd(a,b) = 1`, then `x` is the
/// modular multiplicative inverse of `a` modulo `b`.
///
/// This function declares a MLIR function that given integers `a`
/// and `b`, returns a MLIR struct with `gcd(a,b)` and the Bézout coefficient
/// `x`. Tipically, the EGCD algorithm returns the Bézout coefficient as is.
/// However, because we actually want to calculate the inverse of a modulo b,
/// we wrap the x coefficient around b if negative (x % b).
fn build_egcd_function<'ctx>(
    module: &Module,
    context: &'ctx Context,
    location: Location<'ctx>,
    func_symbol: &str,
    integer_type: Type,
) -> Result<()> {
    // Pseudocode for calculating the EGCD of two integers `a` and `b`.
    // https://en.wikipedia.org/wiki/Extended_Euclidean_algorithm#Pseudocode.
    //
    // ```
    // (old_r, new_r) := (a, b)
    // (old_s, new_s) := (1, 0)
    //
    // while new_r != 0 do
    //     quotient := old_r / new_r
    //     (old_r, new_r) := (new_r, old_r − quotient * new_r)
    //     (old_s, new_s) := (new_s, old_s − quotient * new_s)
    //
    // old_s is equal to Bézout coefficient X
    // old_r is equal to GCD
    // ```
    //
    // Note that when `b > a`, the first iteration inverts the values. Our
    // implementation does it manually as we already know that `b > a`.
    //
    // The core idea of the method is that `gcd(a,b) = gcd(a,b-a)`, and that
    // `gcd(a,b) = gcd(b,a)`. As an optimization, we can actually substract `a`
    // from `b` as many times as possible, so `gcd(a,b) = gcd(b%a,a)`.
    //
    // Take, for example, `a=21` and `b=54`:
    //
    //   gcd(21, 54)
    // = gcd(12, 21)
    // = gcd(9, 12)
    // = gcd(3, 9)
    // = gcd(0, 3)
    // = 3
    //
    // Thus, the algorithm works by calculating a series of remainders `r` which
    // starts with b,a,... being `r[i]` the remainder of dividing `r[i-2]` by
    // `r[i-1]`. At each step, `r[i]` can be calculated as:
    //
    // r[i] = r[i-2] - r[i-1] * quotient
    //
    // The GCD will be the last non-zero remainder.
    //
    // [54; 21; 12; 9; 3; 0]
    //                 ^
    //
    // See Dr. Katherine Stange's Youtube video for a better explanation on how
    // this works: https://www.youtube.com/watch?v=Jwf6ncRmhPg.
    //
    // The extended algorithm also obtains the Bézout coefficients
    // by calculating a series of coefficients `s`. See Dr. Katherine
    // Stange's Youtube video for a better explanation on how this works:
    // https://www.youtube.com/watch?v=IwRtISxAHY4.

    // Define entry block for function. Receives arguments `a` and `b`.
    let region = Region::new();
    let entry_block = region.append_block(Block::new(&[
        (integer_type, location), // a
        (integer_type, location), // b
    ]));

    // Define loop block for function. Each iteration last two values from each series.
    let loop_block = region.append_block(Block::new(&[
        (integer_type, location), // old_r
        (integer_type, location), // new_r
        (integer_type, location), // old_s
        (integer_type, location), // new_s
    ]));

    // Define end block for function.
    let end_block = region.append_block(Block::new(&[
        (integer_type, location), // old_r
        (integer_type, location), // old_s
    ]));

    let modulus = entry_block.arg(1)?;

    // Jump to loop block from entry block, with initial values.
    // - old_r = b
    // - new_r = a
    // - old_s = 0
    // - new_s = 1
    entry_block.append_operation(cf::br(
        &loop_block,
        &[
            modulus, // b
            entry_block.arg(0)?,
            entry_block.const_int_from_type(context, location, 0, integer_type)?,
            entry_block.const_int_from_type(context, location, 1, integer_type)?,
        ],
        location,
    ));

    // LOOP BLOCK
    {
        let old_r = loop_block.arg(0)?;
        let new_r = loop_block.arg(1)?;
        let old_s = loop_block.arg(2)?;
        let new_s = loop_block.arg(3)?;

        // First calculate quotient of old_r/new_r.
        let quotient = loop_block.append_op_result(arith::divui(old_r, new_r, location))?;

        // Multiply quotient by new_r and new_s.
        let quotient_by_new_r = loop_block.muli(quotient, new_r, location)?;
        let quotient_by_new_s = loop_block.muli(quotient, new_s, location)?;

        // Calculate new values for next iteration.
        // - next_new_r := old_r − quotient * new_r
        // - next_new_s := old_s − quotient * new_s
        let next_new_r =
            loop_block.append_op_result(arith::subi(old_r, quotient_by_new_r, location))?;
        let next_new_s =
            loop_block.append_op_result(arith::subi(old_s, quotient_by_new_s, location))?;

        // Jump to end block if next_new_r is zero.
        let zero = loop_block.const_int_from_type(context, location, 0, integer_type)?;
        let next_new_r_is_zero =
            loop_block.cmpi(context, CmpiPredicate::Eq, next_new_r, zero, location)?;
        loop_block.append_operation(cf::cond_br(
            context,
            next_new_r_is_zero,
            &end_block,
            &loop_block,
            &[new_r, new_s],
            &[new_r, next_new_r, new_s, next_new_s],
            location,
        ));
    }

    // END BLOCK
    {
        let gcd = end_block.arg(0)?;
        let beuzout_coeff = end_block.arg(1)?;

        // A pure implementation of EGCD would return the gcd and Bézout
        // coefficient as they are now. However, since we want to return the
        // Bézout coefficient modulo b, we still need to check if it is
        // negative. In such case, we must simply wrap it around back into
        // [0, MODULUS). This is fine because, in such case,
        // |beuzout_coeff| <= divfloor(MODULUS,2).
        let zero = end_block.const_int_from_type(context, location, 0, integer_type)?;
        let is_negative = end_block
            .append_operation(arith::cmpi(
                context,
                CmpiPredicate::Slt,
                beuzout_coeff,
                zero,
                location,
            ))
            .result(0)?
            .into();
        let wrapped_beuzout_coeff = end_block.addi(beuzout_coeff, modulus, location)?;
        let beuzout_coeff = end_block.append_op_result(arith::select(
            is_negative,
            wrapped_beuzout_coeff,
            beuzout_coeff,
            location,
        ))?;

        let results = end_block.append_op_result(llvm::undef(
            llvm::r#type::r#struct(context, &[integer_type, integer_type], false),
            location,
        ))?;
        let results = end_block.insert_values(context, location, results, &[gcd, beuzout_coeff])?;
        end_block.append_operation(llvm::r#return(Some(results), location));
    }

    let func_name = StringAttribute::new(context, func_symbol);
    module.body().append_operation(llvm::func(
        context,
        func_name,
        TypeAttribute::new(llvm::r#type::function(
            llvm::r#type::r#struct(context, &[integer_type, integer_type], false),
            &[integer_type, integer_type],
            false,
        )),
        region,
        &[(
            Identifier::new(context, "no_inline"), // Adding this attribute significantly improves compilation
            Attribute::unit(context),
        )],
        location,
    ));

    Ok(())
}

/// Builds function for circuit arithmetic operations.
///
/// It builds an mlir function to perform most circuit's arithmetic operations
/// with the exception of the inversion since it is handled separately. This
/// allows us to reduce the amount of inlined operations in the mlir generated,
/// significantly reducing the compilation time of circuits.
///
/// Disclaimer: This function could've been split in three functions, each being
/// responsible of one circuit operation, improving maintainability. It would
/// also avoid having to use a `match` in runtime to select the operation to
/// perform, since its known at compile time. However, it was decided not to go
/// with this approach since it would make compilation time about a 10
/// percent slower in circuit-heavy contracts.
fn build_circuit_arith_operation<'ctx>(
    context: &'ctx Context,
    module: &Module,
    location: Location<'ctx>,
    func_symbol: &str,
) -> Result<()> {
    let func_name = StringAttribute::new(context, func_symbol);
    let u2_ty = IntegerType::new(context, 2).into();
    let u384_ty: Type = IntegerType::new(context, 384).into();
    let u385_ty: Type = IntegerType::new(context, 385).into();
    let u768_ty = IntegerType::new(context, 768).into();

    let region = Region::new();
    let entry_block = region.append_block(Block::new(&[
        (u2_ty, location),
        (u384_ty, location),
        (u384_ty, location),
        (u384_ty, location),
    ]));

    let op_tag = entry_block.arg(0)?;
    let lhs = entry_block.arg(1)?;
    let rhs = entry_block.arg(2)?;
    let modulus = entry_block.arg(3)?;

    let ops = [
        CircuitArithOperationType::Add,
        CircuitArithOperationType::Sub,
        CircuitArithOperationType::Mul,
    ];
    let op_blocks = ops
        .into_iter()
        .map(|op| (op, Block::new(&[])))
        .collect_vec();
    let default_block = region.append_block(Block::new(&[]));
    let cases_values = ops.iter().map(|&op| op as i64).collect_vec();

    // Default block. This should be unreachable as the op_tag is not defined by the user.
    {
        // Arithmetic operations' tag go from 0 to 2 (add, sub, mul)
        default_block.append_operation(llvm::unreachable(location));
    }

    // Switch cases' operation blocks.
    for (tag, block) in op_blocks.iter() {
        let result = match tag {
            // result = lhs_value + rhs_value
            CircuitArithOperationType::Add => {
                // We need to extend the operands to avoid overflows while
                // operating. Since we are performing an addition, we need
                // at least a bit width of 384 + 1.
                let lhs = block.extui(lhs, u385_ty, location)?;
                let rhs = block.extui(rhs, u385_ty, location)?;
                let modulus = block.extui(modulus, u385_ty, location)?;

                let result = block.addi(lhs, rhs, location)?;

                // result % circuit_modulus
                block.append_op_result(arith::remui(result, modulus, location))?
            }
            // result = output_value + circuit_modulus - rhs_value
            CircuitArithOperationType::Sub => {
                // We need to extend the operands to avoid overflows while
                // operating. Since we are performing a subtraction, we
                // need at least a bit width of 384 + 1.
                let lhs = block.extui(lhs, u385_ty, location)?;
                let rhs = block.extui(rhs, u385_ty, location)?;
                let modulus = block.extui(modulus, u385_ty, location)?;

                let partial_result = block.addi(lhs, modulus, location)?;
                let result = block.subi(partial_result, rhs, location)?;

                // result % circuit_modulus
                block.append_op_result(arith::remui(result, modulus, location))?
            }
            // result = lhs_value * rhs_value
            CircuitArithOperationType::Mul => {
                // We need to extend the operands to avoid overflows while
                // operating. Since we are performing a multiplication, we need at least a bit width
                // of 284 * 2.
                let lhs = block.extui(lhs, u768_ty, location)?;
                let rhs = block.extui(rhs, u768_ty, location)?;
                let modulus = block.extui(modulus, u768_ty, location)?;

                let result = block.muli(lhs, rhs, location)?;

                // result % circuit_modulus
                block.append_op_result(arith::remui(result, modulus, location))?
            }
        };

        // Truncate back
        let result = block.trunci(result, u384_ty, location)?;

        block.append_operation(llvm::r#return(Some(result), location));
    }

    entry_block.append_operation(cf::switch(
        context,
        &cases_values,
        op_tag,
        u2_ty,
        (&default_block, &[]),
        &op_blocks
            .iter()
            .map(|(_, block)| (block, [].as_slice()))
            .collect::<Vec<_>>(),
        location,
    )?);

    // We need to append the cases to the region.
    for (_, block) in op_blocks.into_iter() {
        region.append_block(block);
    }

    module.body().append_operation(llvm::func(
        context,
        func_name,
        TypeAttribute::new(llvm::r#type::function(
            u384_ty,
            &[u2_ty, u384_ty, u384_ty, u384_ty],
            false,
        )),
        region,
        &[(
            Identifier::new(context, "no_inline"),
            Attribute::unit(context),
        )],
        location,
    ));

    Ok(())
}
