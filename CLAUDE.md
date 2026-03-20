# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Open Uptime Bot is a Rust backend for uptime monitoring with Ntfy.sh push notifications. Clients (like the Pico W microcontroller) ping the server periodically; if pings stop, the server sends "down" notifications.

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

# Local development (builds docker image, resets DB, starts services, builds CLI)
source ./setup_local.sh
```

## Architecture

### Core Components

- **src/main.rs** - Entry point, launches Rocket server and background tasks
- **src/api.rs** - REST endpoints (see API section below)
- **src/context.rs** - In-memory state (`Context`) with `RwLock<HashMap>` for users, tokens, uptime states
- **src/bauth.rs** - Bearer token authentication (`Authorization: token <token>`), IP-based rate limiting (5 req/sec fairing), auth failure logging for fail2ban
- **src/db.rs** - Diesel ORM models and queries
- **src/ntfy.rs** - Ntfy.sh notification integration
- **src/prom.rs** - Prometheus metrics collection
- **src/actions.rs** - Business logic for user/invite creation
- **cli/src/main.rs** - CLI tool (`oubot-cli`) for server management

### Background Tasks (spawned in main.rs)

1. **background_handle_down** - Monitors uptime states, triggers "down" notifications after timeout

### State Management

In-memory HashMap cache backed by PostgreSQL. On startup, loads all users/states from DB into memory. API operations update both in-memory state and database.

### Database Tables

- **users** - Accounts with access_token, up_delay, language_code, ntfy_id
- **uptime_states** - Device status (uninitialized/up/down/paused), touched_at, state_changed_at
- **ntfy_users** - Ntfy.sh credentials per user
- **invites** - Invitation tokens for user registration (is_used tracks consumption)

### API Endpoints

- `GET /api/v1/up` - Client heartbeat (BAuth)
- `GET /api/v1/health` - Health check
- `GET /api/v1/metrics` - Prometheus metrics
- `POST /api/v1/users` - Create user (first admin needs no invite; subsequent users need invite token)
- `POST /api/v1/invites` - Create invite (AdminAuth)
- `GET /api/v1/invites` - List invites (AdminAuth)
- `DELETE /api/v1/invites/<id>` - Delete invite (AdminAuth)
- `GET /api/v1/me` - Current user info (BAuth)
- `POST /api/v1/me/regenerate-token` - Regenerate access token (BAuth)
- `GET|PATCH /api/v1/me/ntfy` - Ntfy settings (BAuth)
- `GET|PATCH /api/v1/me/language` - Language setting (BAuth)
- `GET /api/v1/admin/users` - List all users (AdminAuth)
- `GET /api/v1/admin/users/<id>` - Get user (AdminAuth)
- `DELETE /api/v1/admin/users/<id>` - Delete user (AdminAuth)

### Notification Flow

1. Client sends `GET /api/v1/up` with bearer token
2. Server updates uptime state in memory and DB
3. If state changed (Down->Up or Up->Down after timeout), sends notification via Ntfy.sh
4. Duration messages are localized (Ukrainian/English) based on user's language_code

## Testing

Integration tests in `tests/` directory use Python + Nix test harness:

```bash
nix flake check -L  # Runs all NixOS integration tests
```

Test names are defined in `flake.nix` under `checks`. Each test has a `.nix` file in `tests/` with a corresponding script (`.py` or `.sh`) or inline Python testScript. Run `nix eval .#checks.x86_64-linux --apply 'x: builtins.attrNames x'` to list all test names.

Test infrastructure in `tests/lib/`:
- **config.nix** - Shared constants (ports, credentials, tier name)
- **infra.nix** - PostgreSQL + ntfy-sh service definitions
- **services.nix** - Imports infra.nix, adds oubot systemd service
- **ntfy-bootstrap.nix** - Shared ntfy admin setup script fragment
- **primary.nix** - Imports services.nix, sets env vars and packages for single-node tests
- **lib.nix** - Test runner (builds Python/bash test scripts, passes args to NixOS test)

Native tests import `primary.nix` (full stack). Docker E2E imports `infra.nix` directly (runs oubot via Docker container instead of systemd).

## Configuration

Required `.env` variables:
- `NTFY_BASE_URL`, `NTFY_ADMIN_TOKEN`, `NTFY_USER_TIER` - Ntfy.sh integration
- `DATABASE_URL` - PostgreSQL connection

Server config in `Rocket.toml` (port 8080, `ip_header = "X-Forwarded-For"` for reverse proxy IP extraction).

## Security

- **Rate limiting**: IP-based via governor fairing (5 req/sec per IP), covers all endpoints
- **Auth logging**: Failed auth attempts logged with client IP for fail2ban integration (`[AUTH] ip=... result=...`). fail2ban jail defined inline in NixOS deployer config (not shipped here).
- **Metrics endpoint**: `/api/v1/metrics` is unauthenticated. Block in reverse proxy for production (only expose to internal monitoring network)

## CLI Tool

`oubot-cli` -- management CLI built separately (`nix build .#cli`). Subcommands: `init`, `me`, `token`, `ntfy`, `language`, `admin`. Uses env vars `OUBOT_SERVER` and `OUBOT_TOKEN`.

## Key Dependencies

- **rocket 0.5** - Async web framework
- **diesel 2.1** - PostgreSQL ORM
- **governor** - IP-based rate limiting with DashMap
- **fluent-templates** - i18n (locales in `locales/`)

## Pico W Client

`clients/pico-w/blink.py` - MicroPython firmware that periodically pings the server's `/api/v1/up` endpoint.
