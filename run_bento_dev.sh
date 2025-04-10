#!/bin/bash

# Initialize the database schema (assuming there's a script for this)
# You might need to create this script based on the TaskDB schema
# psql -U bento -h localhost -d taskdb -f init_taskdb.sql

# Start the REST API
RUST_LOG=info cargo run --bin rest_api -- --config bento_config.toml &
API_PID=$!

# Start the Executor agent
RUST_LOG=info cargo run --bin agent -- --config bento_config.toml --agent-type executor &
EXEC_PID=$!

# Start the GPU agent
RUST_LOG=info cargo run --bin agent -- --config bento_config.toml --agent-type gpu &
GPU_PID=$!

# Start the Aux agent
RUST_LOG=info cargo run --bin agent -- --config bento_config.toml --agent-type aux &
AUX_PID=$!

echo "Bento is running. Press Ctrl+C to stop."
wait $API_PID

# Cleanup
kill $EXEC_PID $GPU_PID $AUX_PID
docker-compose down