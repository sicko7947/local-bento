#!/bin/bash

echo "Initializing database schema..."
docker exec -i bento-postgres-1 bash -c "PGPASSWORD=postgres psql -U postgres -d taskdb -f -" < crates/taskdb/migrations/1_taskdb.sql
docker exec -i bento-postgres-1 bash -c "PGPASSWORD=postgres psql -U postgres -d taskdb -f -" < crates/taskdb/migrations/2_optimizations.sql
echo "Database schema initialized."