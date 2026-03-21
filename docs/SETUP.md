# Setup Guide

Complete guide for deploying Open Uptime Bot from scratch with Docker, creating accounts, and flashing client devices.

## Prerequisites

- Docker and Docker Compose
- A self-hosted [ntfy.sh](https://ntfy.sh) instance (or the public one)
- A PostgreSQL-compatible setup (included via Docker Compose)
- [Nix](https://nixos.org/) with flakes enabled (for building images and using the CLI)

## 1. Build the Docker image

```bash
nix build .#docker
docker load < result
```

## 2. Configure ntfy.sh

If you're running your own ntfy.sh instance, set up an admin user and tier:

```bash
# Create admin user (will prompt for password)
ntfy user add --role=admin <name>

# Generate an API token for the admin user
ntfy token add <name>
# Save the printed token — you'll need it below.

# Create a tier for bot-managed users
ntfy tier add \
  --name="basic" \
  --message-limit=1000 \
  --message-expiry-duration=24h \
  --reservation-limit=0 \
  --attachment-file-size-limit=100M \
  --attachment-total-size-limit=1G \
  --attachment-expiry-duration=12h \
  --attachment-bandwidth-limit=5G \
  open-uptime-bot-basic
```

## 3. Create the `.env` file

```bash
# PostgreSQL (used by both the db container and the server)
POSTGRES_USER=oubot
POSTGRES_PASSWORD=<pick-a-password>
POSTGRES_DB=oubot
DATABASE_URL=postgres://oubot:<same-password>@db:5432/oubot

# Ntfy.sh integration
NTFY_BASE_URL=http://your-ntfy-host:port   # e.g. http://ntfy:8091
NTFY_ADMIN_TOKEN=<token-from-step-2>
NTFY_USER_TIER=open-uptime-bot-basic
```

## 4. Start the services

```bash
docker compose up -d
```

This starts PostgreSQL and the Open Uptime Bot server. The server automatically runs database migrations on startup. Verify it's healthy:

```bash
curl http://localhost:8080/api/v1/health
# Expected: {"status":200}
```

## 5. Create the admin account

The first user created (without an invite token) becomes the admin:

```bash
export OUBOT_SERVER=http://localhost:8080

nix develop -c oubot-cli init
```

This prints your admin access token. Save it:

```bash
export OUBOT_TOKEN=<printed-token>
```

Verify:

```bash
nix develop -c oubot-cli me
```

## 6. Create a regular user

Generate an invite token, then use it to create a user:

```bash
# As admin: create an invite
nix develop -c oubot-cli admin create-invite
# Prints: Invite token: <invite-token>

# Create a new user with that invite
nix develop -c oubot-cli init --invite <invite-token>
# Prints the new user's access token
```

Save the user's access token — this is what goes on the client device.

## 7. Configure and flash the client device

### ESP32-C3

See [usage/esp32-c3.md](usage/esp32-c3.md) for the full ESP32-C3 setup guide (build, flash, verify).

### Pico W

See [usage/pico-w.md](usage/pico-w.md) for the full Pico W setup guide (build, flash, verify).

Both clients use compile-time configuration via environment variables and `nix build --impure`. The device connects to WiFi and pings `GET /api/v1/up` every ~5 seconds. If pings stop for longer than the user's `up_delay` (default 30s), the server sends a "power off" notification via ntfy.sh. When pings resume, it sends a "power on" notification with the duration of the outage.

## 8. Subscribe to notifications on your phone

Each user gets an auto-generated ntfy topic and credentials. To receive notifications on your phone:

1. Install the [ntfy app](https://ntfy.sh) (Android: Play Store / F-Droid, iOS: App Store)
2. In the app, add your self-hosted ntfy server (Settings → manage users/server URL)
3. Get your ntfy credentials:
   ```bash
   nix develop -c oubot-cli ntfy show
   # Shows: Enabled, Topic, Username, Password
   ```
4. Subscribe to the topic shown by the command above, using the displayed username and password for authentication

Notifications will appear on your phone when the device goes down or comes back up.

## 9. Manage notifications

```bash
# Disable/enable notifications
nix develop -c oubot-cli ntfy disable
nix develop -c oubot-cli ntfy enable

# Change notification language (uk or en)
nix develop -c oubot-cli language en
```

## 10. Regenerate tokens

If a token is compromised:

```bash
nix develop -c oubot-cli token regenerate
```

This invalidates the old token immediately. You'll need to update the device firmware with the new token and re-flash.

## Using oubot-cli from inside Docker

The CLI is included in the Docker image. You can run commands directly:

```bash
docker exec open-uptime-bot oubot-cli \
  --server http://localhost:8080 \
  init
```

Or interactively:

```bash
docker exec -it open-uptime-bot sh
export OUBOT_SERVER=http://localhost:8080
oubot-cli init
```
