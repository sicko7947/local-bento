# Proto Registry

This directory contains Protocol Buffer (protobuf) definitions for the Local Bento system.

## Directory Structure

- `bento/v1/`: Contains the main service definitions for Local Bento
  - `bento_task_service.proto`: The main gRPC service definition for task management

- `google/protobuf/`: Standard Google protobuf definitions (imported)

## Guidelines for Adding New Protos

1. Place service definitions in the appropriate namespace directory
2. Use versioning in directory names (e.g., `v1`, `v2`) for backward compatibility
3. Follow standard protobuf naming conventions:
   - Use `snake_case` for field names
   - Use `CamelCase` for message and service names
   - Use `CAPITAL_SNAKE_CASE` for enum values

## Compiling Protos

Proto files are compiled to Rust code via build scripts in the respective crates:

- `grpc-client/build.rs`

## Dependencies

We use the following protobuf versions:

- Protocol Buffers: 3
- gRPC: 1.0+
