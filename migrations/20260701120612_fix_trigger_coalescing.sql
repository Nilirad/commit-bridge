DROP INDEX IF EXISTS idx_trigger_queue_pending_target;
CREATE UNIQUE INDEX idx_trigger_queue_pending_target ON trigger_queue (target_repo, event_type) WHERE status = 'PENDING';
