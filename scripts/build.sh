#!/bin/bash


echo "Starting infrastructure..."
docker-compose up -d

# Wait for services to be ready
echo "Waiting for services to start..."
sleep 3

echo "Initializing database schema..."
docker exec -i local-bento-postgres-1 bash -c "PGPASSWORD=postgres psql -U postgres -d taskdb -f -" < crates/taskdb/migrations/1_taskdb.sql
docker exec -i local-bento-postgres-1 bash -c "PGPASSWORD=postgres psql -U postgres -d taskdb -f -" < crates/taskdb/migrations/2_optimizations.sql
echo "Database schema initialized."

# Define configuration parameters
export DATABASE_URL="postgres://postgres:postgres@localhost:5432/taskdb"

# Build the REST API
cargo build --bin rest_api --release 

# Build the agent
cargo build --bin agent --release 

# Build the CLI
cargo build --bin simple_cli --release 

echo "- REST API: http://localhost:8080"
echo "- MinIO Console: http://localhost:9001 (login: minioadmin/minioadmin)"
echo "- PostgreSQL: localhost:5432 (login: bento/bentopassword)"
echo "Press Ctrl+C to stop."

# Setup trap to clean up processes on exit
trap "echo 'Shutting down...'; docker-compose down" EXIT INT TERM