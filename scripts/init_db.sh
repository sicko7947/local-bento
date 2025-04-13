#!/bin/bash

echo "Initializing database schema..."
docker exec -i local_bento_postgres_1 bash -c "PGPASSWORD=postgres psql -U postgres -d taskdb -f -" < crates/taskdb/migrations/1_taskdb.sql
docker exec -i local_bento_postgres_1 bash -c "PGPASSWORD=postgres psql -U postgres -d taskdb -f -" < crates/taskdb/migrations/2_optimizations.sql
echo "Database schema initialized."