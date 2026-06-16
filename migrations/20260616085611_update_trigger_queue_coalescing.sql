-- Add new columns to trigger_queue
ALTER TABLE trigger_queue ADD COLUMN target_repo TEXT;
ALTER TABLE trigger_queue ADD COLUMN event_type TEXT;
ALTER TABLE trigger_queue ADD COLUMN gh_app_installation_id INTEGER;

-- Populate new columns by joining with subscribers
UPDATE trigger_queue
SET target_repo = s.target_repo,
    event_type = s.event_type,
    gh_app_installation_id = s.gh_app_installation_id
FROM subscribers s
WHERE trigger_queue.branch_id = s.branch_id;

-- Create unique index for pending events to enforce coalescing
-- We include branch_id in the unique constraint to ensure we coalesce per subscriber
CREATE UNIQUE INDEX idx_trigger_queue_pending_target ON trigger_queue (branch_id, target_repo, event_type) WHERE status = 'PENDING';
