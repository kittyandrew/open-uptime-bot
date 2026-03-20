# ESP32 Client, Security Hardening, and Metrics Spec

Spec for three phases of work: server-side security + observability, ESP32 Rust client, and Pico W migration.

## Context

### Current State
- Server: Rust/Rocket, PostgreSQL, ntfy.sh notifications, Docker deployment on NixOS (tustan)
- Client: Pico W running MicroPython (`clients/pico-w/blink.py`), HTTP/1.0 GET every ~5s, new TLS handshake per request
- Auth: Bearer token (`Authorization: token tk_...`), IP-based rate limit (5 req/sec via governor fairing), auth failure logging for fail2ban
- Monitoring: Prometheus metrics (`oubot_` prefix) — request-level (total, per-endpoint, latency) + domain-level (uptime state, last seen, auth failures, notifications, active users)
- Testing: 7 NixOS integration tests (Python + bash) + route-guard-lint, including security-auth (fail2ban) and docker-e2e
- Production: Docker containers on tustan via `meow-containers` module, Caddy reverse proxy, fail2ban for SSH + mailu + oubot

### Known Vulnerabilities
- **TLS CERT_NONE**: Pico W client disables certificate verification (`blink.py:40`), making it vulnerable to MITM token capture. Deferred to Phase 3 (Rust rewrite with proper TLS).
- **Unauthenticated metrics**: `/api/v1/metrics` is publicly accessible. Currently harmless (only request counters). Becomes a concern when user-level metrics are added (Phase 1.3). Production mitigation: block the path in Caddy so it's only reachable from the internal monitoring network.

### Production Infrastructure (kittyos/tustan)
- **Reverse proxy**: Caddy on `caddynet`, vhost `oubot.kittyandrew.dev` -> `open-uptime-bot:8080`
- **Networking**: `open-uptime-bot` container on dedicated internal Docker network (`--internal`), connected to `caddynet` via postStart
- **Logging**: All containers log to journald (`log-driver = "journald"`)
- **fail2ban**: Enabled with bantime-increment (1h base, 168h max). Pattern: systemd backend, journalmatch on container unit, iptables DOCKER-USER chain
- **Monitoring**: Prometheus scrapes on internal monitoring network. Grafana with alerting via grafana-to-ntfy
- **Hardening**: `--cap-drop=ALL`, `--security-opt=no-new-privileges`, `--memory` limits

---

## Phase 1: Security Hardening + Observability

Server-side changes only. No client changes.

### 1.1 IP-Aware Auth Logging

**Problem**: `bauth.rs` returns 401/403/429 but logs nothing. Failed auth attempts are invisible. Rate limiter is keyed by user UUID (only throttles authenticated users), not by IP.

**Changes**:

#### Rocket.toml
Add `ip_header` for reverse proxy awareness. Using `X-Forwarded-For` because Caddy sets it by default (no kittyos Caddy changes needed). Rocket parses the first (leftmost) IP from the chain. When no proxy is present (tests), `client_ip()` falls back to the TCP peer address.
```toml
[global]
port = 8080
address = "0.0.0.0"
ip_header = "X-Forwarded-For"
```

#### bauth.rs
Log all auth failures with client IP. Use `Request::client_ip()` (respects `ip_header` config). Format for fail2ban parseability:

```
WARN [AUTH] ip=203.0.113.50 result=invalid_token prefix=tk_...
WARN [AUTH] ip=203.0.113.50 result=missing_header prefix=none
WARN [AUTH] ip=203.0.113.50 result=rate_limited prefix=none
WARN [AUTH] ip=unknown result=no_client_ip — check reverse proxy configuration
```

Log handling for token prefix (static strings only — never logs actual token characters):
- Header has `token tk_` with sufficient length -> `prefix=tk_...`
- Header has `token tk_` but too short -> `prefix=tk_short`
- Header has `token ` but no `tk_` prefix -> `prefix=malformed`
- No Authorization header -> `prefix=none`
- The `prefix=` field is for human debugging only; the fail2ban regex matches on `ip=` and `result=` exclusively.

#### IP-based rate limiting (replaces per-UUID limiter)
Replace the existing `governor::RateLimiter<Uuid, ...>` with a single `RateLimiter<IpAddr, ...>` implemented as a Rocket fairing. This:
- Covers ALL endpoints including unauthenticated ones (e.g., `POST /api/v1/users` invite brute-force)
- Simplifies the code (one limiter instead of two)
- Fires before any endpoint logic
- Configure at ~5 req/sec per IP (sufficient for multi-device NAT, tight enough for security)

Remove the per-UUID rate limiter from `BAuth` and `AdminAuth` guards.

### 1.2 Token Size Reduction

Reduce access token length from 32 to 16 alphanumeric characters (`tk_` + 16 = 19 chars total, ~95 bits entropy). With IP-based fail2ban banning after 10 failed attempts, brute force is impractical at any reasonable entropy level.

Affects: `db.rs` (`User::new`), `db.rs` (`regenerate_user_token`).

Existing tokens are not migrated — they continue working at their current length. Only newly generated tokens use the shorter format.

### 1.3 fail2ban Integration

No filter file ships with oubot — the jail + filter are defined inline in NixOS config, same pattern as kittyos's `mailu-smtp-auth`. The server's job is to emit parseable `[AUTH]` log lines; the deployer configures fail2ban.

#### kittyos integration (downstream -- not built here)
The fail2ban jail goes in `kittyos/system/selfhosted/open-uptime-bot/default.nix`. Example config for reference:
```nix
services.fail2ban.jails.oubot-auth = {
  filter = {
    Definition.failregex = "^.* \\[AUTH\\] ip=<HOST> result=(invalid_token|missing_header)";
    Init.journalmatch = "_SYSTEMD_UNIT=docker-open-uptime-bot.service";
  };
  settings = {
    enabled = true;
    backend = "systemd";
    maxretry = 10;
    findtime = "5m";
    bantime = "1h";
    action = ''iptables-multiport[name=oubot, port="80,443", chain=DOCKER-USER]'';
  };
};
```
Similarly, the Prometheus scrape config and monitoring network join go in kittyos.

### 1.4 Domain-Level Prometheus Metrics

**@WARNING**: `/api/v1/metrics` is unauthenticated. Adding `user_id` labels exposes UUIDs and device status publicly. In production, block this path in Caddy so only the internal monitoring network can reach it. This matches the ntfy pattern (metrics only on monitoring network).

Add to `src/prom.rs`:

| Metric | Type | Labels | Purpose |
|--------|------|--------|---------|
| `oubot_uptime_state` | IntGaugeVec | `user_id` | Current state (0=uninit, 1=up, 2=down, 3=paused) |
| `oubot_last_seen_timestamp` | GaugeVec | `user_id` | Unix timestamp of last heartbeat |
| `oubot_auth_failures_total` | IntCounterVec | `reason` | Auth failures (invalid_token, missing_header, rate_limited) |
| `oubot_notifications_total` | IntCounterVec | `type`, `result` | Notifications sent/failed (increment inside tokio::spawn, after send result is known) |
| `oubot_active_users` | IntGauge | - | Registered user count |

Update callsites:
- `bauth.rs` (fairing): increment `auth_failures_total` on rejection
- `dispatch_notifications` (inside inner `tokio::spawn`): increment `notifications_total` with `result=success` or `result=failure` after `send_notification()` completes
- `api_up`: update `last_seen_timestamp` and `uptime_state` after touch
- `create_user`/`delete_user`: update `active_users`

### 1.5 E2E Test: Auth Security

New NixOS integration test: `tests/security-auth.nix` (inline Python testScript)

**Two-node test** (separate client and server VMs) to avoid banning localhost. Adds fail2ban to the server VM.

Test script flow:
1. Create admin user from client VM (success path)
2. Fire requests with invalid tokens from client VM
3. Verify server logs contain the expected `[AUTH]` format
4. Verify fail2ban detects the pattern and bans the client VM's IP
5. Verify subsequent requests from client VM are dropped
6. Verify IP rate limiter kicks in correctly
7. Check Prometheus metrics: `oubot_auth_failures_total` incremented

### 1.6 Docker E2E Test

New NixOS integration test that runs oubot from the Docker image (same pattern as grafana-to-ntfy's `grafana-docker-test.nix`). Adapts an existing happy-path test to validate the production artifact:

1. Load Docker image via `docker load < ${docker-image}`
2. Run container via systemd oneshot with `--network host` and env vars
3. Run the same test assertions as an existing test (e.g., user creation + ping + notification)

This ensures the Docker image, migrations, and startup sequence all work.

### 1.7 Metrics Assertions in Existing Tests

Add lightweight metric checks (`curl /api/v1/metrics | grep '^oubot_'`) to existing test scripts. Verifies domain metrics update correctly during normal operations. Use exact metric name matching to avoid fragile patterns.

---

## Phase 2: ESP32 Rust Client

### 2.1 Framework: esp-hal

Using esp-hal 1.0.0 (stable, Oct 2025) + esp-radio 0.17.0 + reqwless 0.13.0 with embedded-tls. Espressif officially backs this path. Board: ESP32-C3 (RISC-V), standard Rust toolchain via `riscv32imc-unknown-none-elf` target.

### 2.2 Setup (Completed)

Incremental hardware bring-up: toolchain → blink → WiFi → HTTPS → full client. All steps verified on real hardware.

### 2.3 Client Architecture

```
clients/esp32/
  .cargo/
    config.toml       # Target, rustflags, build-std (stripped by Nix build)
  Cargo.toml
  Cargo.lock
  build.rs            # Warns on missing/empty env vars, sets cargo:rerun-if-env-changed
  rust-toolchain.toml # Nightly + riscv32imc target + rust-src
  src/
    main.rs           # WiFi connect, heartbeat loop, LED feedback
```

#### Compile-time configuration
All config baked in via `env!()` macros populated by Nix build or manual env vars:
- `OUBOT_WIFI_SSID` — WiFi network name
- `OUBOT_WIFI_PASS` — WiFi password
- `OUBOT_SERVER` — Server URL (e.g. `https://oubot.kittyandrew.dev`)
- `OUBOT_TOKEN` — Access token (e.g. `tk_abcdef1234567890`)

Token/WiFi changes require rebuilding + re-flashing. This is acceptable for the current use case. WiFi AP provisioning is a future enhancement.

#### Client behavior
1. Connect to WiFi (retry with backoff on failure)
2. Establish persistent HTTPS connection to server (TLS encrypted, no cert verification — `TlsVerify::None`; reqwless 0.14+ adds `TlsVerify::Certificate` but requires embassy-net ecosystem upgrade)
3. Send `GET /api/v1/up` with `Authorization: token <TOKEN>` every ~5 seconds
4. On connection drop: reconnect (WiFi first, then HTTPS)
5. LED feedback: blink on ping, solid on error, off during sleep

Key improvement over Pico W: persistent HTTPS connection via `reqwless::HttpResource` eliminates per-ping TLS handshake overhead. Single handshake at connect; heartbeats reuse the session. On connection drop, reconnects with fresh TLS.

### 2.4 Nix Build Integration

Add to `flake.nix`. Exact shape depends on the ESP32 variant (toolchain differences), worked out during the setup session (2.2).

### 2.5 USAGE.md (Completed)

End-user guide for invite token holders. Written after the ESP32 client implementation stabilizes. Covers:
1. Prerequisites: ESP32 board, USB cable, WiFi credentials, invite token
2. Getting your access token: `oubot-cli init --invite <token>`
3. Building the firmware with your config
4. Flashing the device
5. Verifying it works (LED behavior, `oubot-cli me`)
6. Managing notifications: enable/disable, language
7. Token regeneration (requires re-flash)
8. Troubleshooting

---

## Phase 3: Pico W Migration to Rust

### 3.1 Scope

Rewrite `clients/pico-w/blink.py` in Rust using `embassy-rp` + `cyw43` (WiFi driver). Fixes the TLS CERT_NONE vulnerability. The Pico W (RP2040, 264KB RAM) is more constrained than ESP32 but has good Rust support.

### 3.2 Approach

- Share patterns with the ESP32 client (HTTP logic, retry patterns, `env!()` config)
- Proper TLS certificate validation (fixes the CERT_NONE gap)
- Compile-time configuration via same pattern
- Add to `flake.nix` as `packages.pico-w-client`
- Update USAGE.md with Pico W instructions
- Keep MicroPython client as reference (don't delete yet)

### 3.3 Dependency

Depends on Phase 2. Same patterns apply (different HAL, same application logic). If Phase 2 reveals issues with esp-hal, Phase 3 adjusts accordingly.

---

## Resolved Decisions

1. **IP header**: `X-Forwarded-For` (Caddy's default, no kittyos changes needed)
2. **Rate limiter**: Single IP-based limiter as Rocket fairing, replaces per-UUID limiter. ~5 req/sec per IP.
3. **Metrics endpoint security**: Keep unprotected but add `@WARNING` comment. Production: block in Caddy, only reachable from monitoring network.
4. **Token size**: 16 alphanumeric chars (`tk_` + 16 = 19 total, ~95 bits). No migration of existing tokens.
5. **fail2ban test**: Two-node NixOS test (separate client/server VMs).
6. **Client testing**: No separate test client binary. Docker E2E test validates the production artifact instead.
7. **ESP32 config**: Compile-time bake everything via `env!()`. WiFi AP provisioning is a future enhancement.
8. **TLS gap**: Noted in spec, deferred to Phase 3 (Pico W Rust rewrite).
9. **Phase 3**: Committed, not conditional.
10. **fail2ban regex**: Starting point provided, must be verified against actual journald output during implementation.
11. **Protocol**: Keep HTTP GET. Efficiency win from persistent connections in Rust client.

12. **Existing metrics**: Rename to `oubot_` prefix for consistency (`oubot_requests_total`, `oubot_request_duration_seconds`, etc.).
13. **Docker image tag**: Date-based convention (`2026.3.20`). kittyos flake input needs bump after changes.

## Open Questions

*None — all resolved.*

## Resolved (Phase 2)

14. **ESP32 variant**: ESP32-C3 (RISC-V, rev v0.4, 4MB flash). Standard Rust toolchain via `riscv32imc-unknown-none-elf` target.
15. **esp-hal WiFi+TLS**: Works. esp-hal 1.0.0 + esp-radio 0.17.0 + reqwless 0.13.0 with embedded-tls. TLS handshake completes in ~170ms. No cert verification (same as Pico W).
16. **GPIO8 LED polarity**: Active-low on this board (HIGH = off, LOW = on).
17. **Nix build**: crane with fenix `combine` (nightly rustc/cargo + pre-built riscv32imc rust-std). `build-std` incompatible with crane dep vendoring; devShell uses `build-std` via `complete.toolchain` with rust-src.
