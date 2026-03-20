-- Add language_code to users table (moved from tg_users to be user-wide)
ALTER TABLE users ADD COLUMN language_code TEXT NOT NULL DEFAULT 'uk';

-- Copy language_code from tg_users to users for existing data
UPDATE users u
SET language_code = t.language_code
FROM tg_users t
WHERE u.tg_id = t.id;

-- Remove Telegram integration entirely
ALTER TABLE users DROP COLUMN tg_id;
DROP TABLE tg_users;
DROP TYPE chat_state_enum;

-- Rename misspelled enum value
ALTER TYPE status_enum RENAME VALUE 'maintainance' TO 'paused';

-- Remove unused down_delay column
ALTER TABLE users DROP COLUMN down_delay;
