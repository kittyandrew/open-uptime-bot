-- Add state_changed_at column to track when status transitions occur
ALTER TABLE uptime_states ADD COLUMN state_changed_at TIMESTAMP DEFAULT now() NOT NULL;
