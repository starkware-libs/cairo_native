use sierra_emu::Value;
use starknet_crypto::Felt;

/// Convert a Value to a felt.
pub fn value_to_felt(value: &Value) -> Vec<Felt> {
    let mut felts = Vec::new();
    match value {
        Value::Array { data, .. } | Value::Struct(data) => {
            data.iter().flat_map(value_to_felt).collect()
        }
        Value::BoundedInt { value, .. } => vec![value.into()],
        Value::Bytes31(bytes) => vec![*bytes],
        Value::BuiltinCosts(costs) => vec![
            costs.r#const.into(),
            costs.pedersen.into(),
            costs.bitwise.into(),
            costs.ecop.into(),
            costs.poseidon.into(),
            costs.add_mod.into(),
            costs.mul_mod.into(),
        ],
        Value::CircuitModulus(value) => vec![value.into()],
        Value::Circuit(data) => data.iter().map(Felt::from).collect(),
        Value::CircuitOutputs {
            circuits: data,
            modulus,
        } => {
            let mut felts = data.iter().map(Felt::from).collect::<Vec<_>>();
            felts.push(modulus.into());
            felts
        }
        Value::EcPoint { x, y } | Value::EcState { x, y } => {
            vec![*x, *y]
        }
        Value::Enum {
            self_ty,
            index,
            payload,
        } => {
            if let Some(debug_name) = &self_ty.debug_name {
                if debug_name == "core::bool" {
                    vec![(*index == 1).into()]
                } else {
                    let mut felts = vec![(*index).into()];
                    felts.extend(value_to_felt(payload));
                    felts
                }
            } else {
                // Assume its a regular enum.
                let mut felts = vec![(*index).into()];
                felts.extend(value_to_felt(payload));
                felts
            }
        }
        Value::Felt(felt) => vec![*felt],
        Value::FeltDict { data, .. } => {
            for (key, value) in data {
                felts.push(*key);
                let felt = value_to_felt(value);
                felts.extend(felt);
            }

            felts
        }
        Value::FeltDictEntry {
            key: data_key,
            data,
            ..
        } => {
            felts.push(*data_key);

            for (key, value) in data {
                felts.push(*key);
                let felt = value_to_felt(value);
                felts.extend(felt);
            }

            felts
        }
        Value::I8(x) => vec![(*x).into()],
        Value::I16(x) => vec![(*x).into()],
        Value::I32(x) => vec![(*x).into()],
        Value::I64(x) => vec![(*x).into()],
        Value::I128(x) => vec![(*x).into()],
        Value::U8(x) => vec![(*x).into()],
        Value::U16(x) => vec![(*x).into()],
        Value::U32(x) => vec![(*x).into()],
        Value::U64(x) => vec![(*x).into()],
        Value::U128(x) => vec![(*x).into()],
        Value::U256(x, y) => vec![(*x).into(), (*y).into()],
        Value::IntRange { x, y } => {
            let felt = value_to_felt(x);
            felts.extend(felt);
            let felt = value_to_felt(y);
            felts.extend(felt);
            felts
        }
        Value::Unit | Value::Null | Value::Uninitialized { .. } => vec![0.into()],
    }
}
