-- Recreate Telegram integration tables
CREATE TYPE chat_state_enum AS ENUM ('main', 'invites');
CREATE TABLE tg_users (
  id uuid PRIMARY KEY,
  enabled BOOLEAN DEFAULT true NOT NULL,
  user_id BIGINT,
  chat_id BIGINT,
  chat_state chat_state_enum DEFAULT 'main' NOT NULL,
  language_code TEXT NOT NULL DEFAULT 'uk'
);
ALTER TABLE users ADD COLUMN tg_id uuid REFERENCES tg_users (id) ON DELETE CASCADE;

-- Remove language_code from users table
ALTER TABLE users DROP COLUMN language_code;

-- Restore original enum value name
ALTER TYPE status_enum RENAME VALUE 'paused' TO 'maintainance';

-- Restore unused down_delay column
ALTER TABLE users ADD COLUMN down_delay SMALLINT NOT NULL DEFAULT 0;
