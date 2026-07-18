uv run --with grpcio-tools python -m grpc_tools.protoc \
  --proto_path=../../proto \
  --python_out=. \
  --grpc_python_out=. \
  ../../proto/lm_service.proto
