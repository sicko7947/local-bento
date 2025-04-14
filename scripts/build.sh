#!/bin/bash


echo "Starting infrastructure..."
docker-compose up -d --force-recreate

# Wait for services to be ready
echo "Waiting for services to start..."
sleep 3

# Define configuration parameters
export DATABASE_URL="postgres://postgres:postgres@localhost:5432/taskdb"

# Build the REST API
cargo build --bin rest_api --release 

# Build the agent
cargo build --bin agent --release 

# Build the CLI
cargo build --bin bento_cli --release --features="sample-guest-common sample-guest-methods"

echo "- REST API: http://localhost:8080"
echo "- MinIO Console: http://localhost:9001 (login: minioadmin/minioadmin)"
echo "- PostgreSQL: localhost:5432 (login: bento/bentopassword)"
echo "Press Ctrl+C to stop."

# Setup trap to clean up processes on exit
trap "echo 'Shutting down...'; docker-compose down" EXIT INT TERM