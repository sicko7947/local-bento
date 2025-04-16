#!/bin/bash

echo "Starting infrastructure..."
docker compose down --volumes --remove-orphans && docker compose build --no-cache && docker compose up --build -d

# Wait for services to be ready
echo "Waiting for services to start..."
sleep 3

# Define configuration parameters
DATABASE_URL="postgres://postgres:postgres@localhost:5432/taskdb"
REDIS_URL="redis://localhost:6379"
S3_BUCKET="proofs"
S3_ACCESS_KEY="minioadmin"
S3_SECRET_KEY="minioadmin"
S3_URL="http://localhost:9000"

# Start the REST API with all required parameters
echo "Starting REST API..."
RUST_LOG=info ./release/linux_amd64/rest_api -- \
    "$DATABASE_URL" \
    "$S3_BUCKET" \
    "$S3_ACCESS_KEY" \
    "$S3_SECRET_KEY" \
    "$S3_URL" &
API_PID=$!

# Give the API a moment to start up
sleep 2

# Start the Executor agent
echo "Starting Executor agent..."
RUST_LOG=debug ./release/linux_amd64/agent \
    --task-stream exec \
    "$DATABASE_URL" \
    "$REDIS_URL" \
    "$S3_BUCKET" \
    "$S3_ACCESS_KEY" \
    "$S3_SECRET_KEY" \
    "$S3_URL" &
EXEC_PID=$!

# Start the GPU agent
echo "Starting GPU agent..."
RUST_LOG=debug ./release/linux_amd64/agent \
    --task-stream prove \
    --segment-po2 19 \
    "$DATABASE_URL" \
    "$REDIS_URL" \
    "$S3_BUCKET" \
    "$S3_ACCESS_KEY" \
    "$S3_SECRET_KEY" \
    "$S3_URL" &
GPU_PID=$!

# Start the Aux agent
echo "Starting Aux agent..."
RUST_LOG=debug ./release/linux_amd64/agent \
    --task-stream aux \
    "$DATABASE_URL" \
    "$REDIS_URL" \
    "$S3_BUCKET" \
    "$S3_ACCESS_KEY" \
    "$S3_SECRET_KEY" \
    "$S3_URL" &
AUX_PID=$!

echo "Bento is running with:"
echo "- REST API: http://localhost:8080"
echo "- MinIO Console: http://localhost:9001 (login: minioadmin/minioadmin)"
echo "- PostgreSQL: localhost:5432 (login: postgres/postgres)"
echo "Press Ctrl+C to stop."

# Setup trap to clean up processes on exit
trap "echo 'Shutting down...'; kill $API_PID $EXEC_PID $GPU_PID $AUX_PID; docker-compose down" EXIT INT TERM

# Wait for the API process to finish
wait $API_PID
