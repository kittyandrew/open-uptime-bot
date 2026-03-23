-- Maintenance window: minutes from midnight UTC (0-1439), both NULL = disabled
ALTER TABLE users ADD COLUMN maint_window_start_utc SMALLINT DEFAULT NULL;
ALTER TABLE users ADD COLUMN maint_window_end_utc SMALLINT DEFAULT NULL;
ALTER TABLE users ADD CONSTRAINT maint_window_both_or_neither
    CHECK ((maint_window_start_utc IS NULL) = (maint_window_end_utc IS NULL));
ALTER TABLE users ADD CONSTRAINT maint_window_valid_range
    CHECK (
        maint_window_start_utc IS NULL
        OR (maint_window_start_utc >= 0 AND maint_window_start_utc < 1440
            AND maint_window_end_utc >= 0 AND maint_window_end_utc < 1440
            AND maint_window_start_utc != maint_window_end_utc)
    );

-- Pre-pause status: tracks what state to restore on unpause
ALTER TABLE uptime_states ADD COLUMN pre_pause_status status_enum DEFAULT NULL;

-- Bump existing users from 30s to 60s (only those who never customized)
UPDATE users SET up_delay = 60 WHERE up_delay = 30;
