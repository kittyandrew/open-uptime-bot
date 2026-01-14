# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Open Uptime Bot is a Rust backend for uptime monitoring with Ntfy.sh and Telegram notification integrations. Clients (like the Pico W microcontroller) ping the server periodically; if pings stop, the server sends "down" notifications.

## Build Commands

```bash
# Development environment (all tools: Rust, Diesel CLI, Python, Pico SDK)
nix develop

# Run tests
nix flake check -L

# Build Docker image
nix build .#docker
docker load < result

# Database operations
./diesel_run.sh       # Run migrations
./recreate_db.sh      # Reset database
```

## Architecture

### Core Components

- **src/main.rs** - Entry point, launches Rocket server and background tasks
- **src/api.rs** - REST endpoints (`/api/v1/up`, `/api/v1/users`, `/api/v1/invites`, `/api/v1/health`, `/api/v1/metrics`)
- **src/context.rs** - In-memory state (`Context`) with `RwLock<HashMap>` for users, tokens, uptime states
- **src/bauth.rs** - Bearer token authentication (`Authorization: token <token>`) and per-user rate limiting (2 req/sec)
- **src/db.rs** - Diesel ORM models and queries
- **src/ntfy.rs** - Ntfy.sh notification integration
- **src/tg.rs** - Telegram bot (Grammers MTProto client)
- **src/prom.rs** - Prometheus metrics collection
- **src/actions.rs** - Business logic for user/invite creation

### Background Tasks (spawned in main.rs)

1. **background_handle_down** - Monitors uptime states, triggers "down" notifications after timeout
2. **background_handle_telegram** - Processes Telegram messages and callbacks

### State Management

In-memory HashMap cache backed by PostgreSQL. On startup, loads all users/states from DB into memory. API operations update both in-memory state and database.

### Database Tables

- **users** - Accounts with access_token, up_delay, down_delay, ntfy_id, tg_id
- **uptime_states** - Device status (uninitialized/up/down/maintainance), touched_at timestamps
- **ntfy_users** - Ntfy.sh credentials per user
- **tg_users** - Telegram user mappings with chat_state and language_code
- **invites** - Invitation tokens for user registration

### Notification Flow

1. Client sends `GET /api/v1/up` with bearer token
2. Server updates uptime state in memory and DB
3. If state changed (Down→Up or Up→Down after timeout), sends notification via Ntfy.sh and/or Telegram

## Testing

Integration tests in `tests/` directory use Python + Nix test harness:

```bash
nix flake check  # Runs all tests including api-v1-up-test-success
```

Test flow: create user → connect to Ntfy.sh WebSocket → send pings → wait for notifications → verify timing.

## Configuration

Required `.env` variables:
- `NTFY_BASE_URL`, `NTFY_ADMIN_TOKEN`, `NTFY_USER_TIER` - Ntfy.sh integration
- `GRAMMERS_BOT_TOKEN`, `GRAMMERS_API_ID`, `GRAMMERS_API_HASH`, `TELEGRAM_SUPERUSER_ID` - Telegram bot
- `DATABASE_URL` - PostgreSQL connection

Server config in `Rocket.toml` (port 8080).

## Key Dependencies

- **rocket 0.5** - Async web framework
- **diesel 2.1** - PostgreSQL ORM
- **grammers-client** - Telegram MTProto
- **governor** - Rate limiting with DashMap
- **fluent-templates** - i18n (locales in `locales/`)

## Pico W Client

`clients/pico-w/blink.py` - MicroPython firmware that periodically pings the server's `/api/v1/up` endpoint.
