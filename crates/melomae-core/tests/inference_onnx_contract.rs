//! The ONNX adapter returns a typed error for a wrong output contract.
//!
//! This test builds two small ONNX models in memory with a minimal protobuf
//! writer: one whose graph output is named `bogus`, and one whose output has two
//! channels. It writes them into a temporary directory, and the guard removes
//! the directory when the test ends. Nothing is committed.
//!
//! Before the fix, the wrong-name model panicked on the output lookup and the
//! wrong-shape model silently returned two channels.

mod common;
mod setup;

use std::sync::atomic::AtomicBool;

use melomae_core::inference::{load_onnx_backend, InferenceError, RestoreBackend, RestoreSettings};

use setup::TempDir;

// --- a minimal ONNX protobuf writer -----------------------------------------

/// A tensor dimension: a fixed value or a symbolic parameter.
enum Dim<'a> {
    Value(i64),
    Param(&'a str),
}

/// Protobuf base-128 varint.
fn varint(mut value: u64) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            out.push(byte);
            break;
        }
        out.push(byte | 0x80);
    }
    out
}

/// A protobuf field key: the field number and the wire type.
fn key(field: u32, wire: u8) -> Vec<u8> {
    varint((u64::from(field) << 3) | u64::from(wire))
}

/// A length-delimited field from raw bytes.
fn field_bytes(field: u32, body: &[u8]) -> Vec<u8> {
    let mut out = key(field, 2);
    out.extend(varint(body.len() as u64));
    out.extend(body);
    out
}

/// A length-delimited field from a string.
fn field_str(field: u32, value: &str) -> Vec<u8> {
    field_bytes(field, value.as_bytes())
}

/// A varint field.
fn field_varint(field: u32, value: u64) -> Vec<u8> {
    let mut out = key(field, 0);
    out.extend(varint(value));
    out
}

/// One `TensorShapeProto.Dimension`.
fn dimension(dim: &Dim) -> Vec<u8> {
    match dim {
        Dim::Value(value) => field_varint(1, *value as u64),
        Dim::Param(name) => field_str(2, name),
    }
}

/// A `TensorShapeProto`.
fn tensor_shape(dims: &[Dim]) -> Vec<u8> {
    let mut out = Vec::new();
    for dim in dims {
        out.extend(field_bytes(1, &dimension(dim)));
    }
    out
}

/// A `TypeProto` for a float tensor with the given shape.
fn tensor_type(dims: &[Dim]) -> Vec<u8> {
    // TypeProto.tensor_type, then Tensor.elem_type = FLOAT (1), then shape.
    let mut tensor = field_varint(1, 1);
    tensor.extend(field_bytes(2, &tensor_shape(dims)));
    field_bytes(1, &tensor)
}

/// A `ValueInfoProto`.
fn value_info(name: &str, dims: &[Dim]) -> Vec<u8> {
    let mut out = field_str(1, name);
    out.extend(field_bytes(2, &tensor_type(dims)));
    out
}

/// An `AttributeProto` with an integer value.
fn attribute_int(name: &str, value: i64) -> Vec<u8> {
    let mut out = field_str(1, name);
    out.extend(field_varint(20, 2)); // AttributeProto.AttributeType.INT
    out.extend(field_varint(3, value as u64));
    out
}

/// A `NodeProto`.
fn node(op_type: &str, inputs: &[&str], output: &str, attributes: &[Vec<u8>]) -> Vec<u8> {
    let mut out = Vec::new();
    for input in inputs {
        out.extend(field_str(1, input));
    }
    out.extend(field_str(2, output));
    out.extend(field_str(3, "node"));
    out.extend(field_str(4, op_type));
    for attribute in attributes {
        out.extend(field_bytes(5, attribute));
    }
    out
}

/// A `GraphProto` with one input and one output.
fn graph(node: &[u8], input: &[u8], output: &[u8]) -> Vec<u8> {
    let mut out = field_bytes(1, node);
    out.extend(field_str(2, "test_graph"));
    out.extend(field_bytes(11, input));
    out.extend(field_bytes(12, output));
    out
}

/// A `ModelProto` with IR version 8 and the ai.onnx opset 17.
fn model(graph: &[u8]) -> Vec<u8> {
    let mut out = field_varint(1, 8); // ir_version
    out.extend(field_str(2, "melomae-test")); // producer_name
    let opset = field_varint(2, 17); // OperatorSetIdProto.version
    out.extend(field_bytes(8, &opset)); // opset_import
    out.extend(field_bytes(7, graph));
    out
}

/// An Identity graph whose output is named `output_name`.
fn identity_model(output_name: &str) -> Vec<u8> {
    let input = value_info("audio", &[Dim::Value(1), Dim::Value(1), Dim::Param("N")]);
    let output = value_info(
        output_name,
        &[Dim::Value(1), Dim::Value(1), Dim::Param("N")],
    );
    let node = node("Identity", &["audio"], output_name, &[]);
    model(&graph(&node, &input, &output))
}

/// A Concat graph whose output has two channels. The node joins the input with
/// itself along the channel axis.
fn two_channel_model() -> Vec<u8> {
    let input = value_info("audio", &[Dim::Value(1), Dim::Value(1), Dim::Param("N")]);
    let output = value_info(
        "audio_out",
        &[Dim::Value(1), Dim::Value(2), Dim::Param("N")],
    );
    let node = node(
        "Concat",
        &["audio", "audio"],
        "audio_out",
        &[attribute_int("axis", 1)],
    );
    model(&graph(&node, &input, &output))
}

/// Load a backend from in-memory model bytes written to a temporary file.
fn backend_from(dir: &TempDir, name: &str, bytes: &[u8]) -> melomae_core::inference::OnnxBackend {
    let path = dir.join(name);
    std::fs::write(&path, bytes).expect("write the constructed ONNX model");
    load_onnx_backend(&path).expect("load the constructed ONNX model")
}

/// Restore one channel through a backend, with the full-file path.
fn restore(backend: &melomae_core::inference::OnnxBackend) -> Result<(), InferenceError> {
    match backend.restore(
        &[vec![0.1; 1_000]],
        &RestoreSettings::default(),
        &mut |_| {},
        &AtomicBool::new(false),
    ) {
        Ok(_) => Ok(()),
        Err(error) => Err(error),
    }
}

#[test]
fn an_inference_model_with_a_wrong_output_name_returns_a_typed_error() {
    let dir = TempDir::new("melomae-onnx-wrong-name");
    let backend = backend_from(&dir, "wrong-output.onnx", &identity_model("bogus"));
    assert!(
        matches!(restore(&backend), Err(InferenceError::Model(_))),
        "a missing output name must return a typed Model error"
    );
}

#[test]
fn an_inference_model_with_a_wrong_output_shape_returns_a_typed_error() {
    let dir = TempDir::new("melomae-onnx-wrong-shape");
    let backend = backend_from(&dir, "wrong-shape.onnx", &two_channel_model());
    assert!(
        matches!(restore(&backend), Err(InferenceError::Model(_))),
        "a wrong output shape must return a typed Model error"
    );
}
