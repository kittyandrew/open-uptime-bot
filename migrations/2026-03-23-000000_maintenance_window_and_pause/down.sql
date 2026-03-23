-- Revert up_delay bump (best-effort: only revert users still at 60)
UPDATE users SET up_delay = 30 WHERE up_delay = 60;

-- Drop pre-pause status
ALTER TABLE uptime_states DROP COLUMN pre_pause_status;

-- Drop maintenance window
ALTER TABLE users DROP CONSTRAINT maint_window_valid_range;
ALTER TABLE users DROP CONSTRAINT maint_window_both_or_neither;
ALTER TABLE users DROP COLUMN maint_window_end_utc;
ALTER TABLE users DROP COLUMN maint_window_start_utc;
