CREATE TYPE chat_state_enum AS ENUM ('main', 'invites');
CREATE TABLE ntfy_users (
  id uuid PRIMARY KEY,
  enabled BOOLEAN NOT NULL,
  topic TEXT NOT NULL,
  topic_permission TEXT NOT NULL,
  username TEXT NOT NULL,
  password TEXT NOT NULL,
  tier TEXT NOT NULL
);

CREATE TABLE tg_users (
  id uuid PRIMARY KEY,
  enabled BOOLEAN DEFAULT true NOT NULL,
  user_id BIGINT NOT NULL,
  chat_id BIGINT,
  chat_state chat_state_enum DEFAULT 'main' NOT NULL,
  language_code TEXT NOT NULL
);

CREATE TYPE user_type_enum AS ENUM ('normal', 'admin');
CREATE TABLE users (
  id uuid PRIMARY KEY,
  created_at TIMESTAMP DEFAULT now() NOT NULL,
  user_type user_type_enum NOT NULL,
  invites_limit BIGINT NOT NULL,
  invites_used BIGINT DEFAULT 0 NOT NULL,
  access_token TEXT NOT NULL,
  up_delay SMALLINT NOT NULL,
  down_delay SMALLINT NOT NULL,
  ntfy_id uuid REFERENCES ntfy_users (id) ON DELETE CASCADE NOT NULL,
  tg_id uuid REFERENCES tg_users (id) ON DELETE CASCADE NOT NULL
);

CREATE TYPE status_enum AS ENUM ('uninitialized', 'up', 'down', 'maintainance');
CREATE TABLE uptime_states (
  id uuid PRIMARY KEY,
  created_at TIMESTAMP DEFAULT now() NOT NULL,
  touched_at TIMESTAMP DEFAULT now() NOT NULL,
  status status_enum DEFAULT 'uninitialized' NOT NULL,
  user_id uuid REFERENCES users (id) ON DELETE CASCADE
);

CREATE TABLE invites (
  id uuid PRIMARY KEY,
  created_at TIMESTAMP DEFAULT now() NOT NULL,
  token TEXT NOT NULL,
  is_used BOOLEAN DEFAULT false NOT NULL,
  owner_id uuid REFERENCES users (id) ON DELETE CASCADE,
  user_id uuid REFERENCES users (id) ON DELETE SET NULL
);
