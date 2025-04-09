-- Create enum for task status
CREATE TYPE task_status AS ENUM ('pending', 'running', 'completed', 'failed');

-- Create proof tasks table
CREATE TABLE IF NOT EXISTS proof_tasks (
    id UUID PRIMARY KEY,
    status task_status NOT NULL,
    image_id TEXT NOT NULL,
    input_data BYTEA,
    segment_count INTEGER NOT NULL,
    segment_size INTEGER NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create proof segments table
CREATE TABLE IF NOT EXISTS proof_segments (
    id UUID PRIMARY KEY,
    task_id UUID NOT NULL REFERENCES proof_tasks(id),
    segment_index INTEGER NOT NULL,
    gpu_id INTEGER,
    status task_status NOT NULL,
    proof BYTEA,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);