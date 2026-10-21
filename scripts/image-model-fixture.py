#!/usr/bin/env python3
"""TEST DATA. Generate a tiny local ONNX model for native inference plumbing, not segmentation quality.
Requires onnx==1.19.1. Rust tract performs the actual inference in the native-model lane.
"""
import sys
from pathlib import Path
import onnx
from onnx import TensorProto, helper

output = Path(sys.argv[1])
output.parent.mkdir(parents=True, exist_ok=True)
graph = helper.make_graph(
    [helper.make_node("Gather", ["input", "channel"], ["mask"], axis=1)],
    "TEST DATA red channel segmentation",
    [helper.make_tensor_value_info("input", TensorProto.FLOAT, [1, 3, 320, 320])],
    [helper.make_tensor_value_info("mask", TensorProto.FLOAT, [1, 1, 320, 320])],
    [helper.make_tensor("channel", TensorProto.INT64, [1], [0])],
)
model = helper.make_model(graph, opset_imports=[helper.make_opsetid("", 17)])
model.ir_version = 10
onnx.checker.check_model(model)
onnx.save(model, output)
