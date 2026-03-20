# Setup Guide

Complete guide for deploying Open Uptime Bot from scratch with Docker, creating accounts, and flashing client devices.

## Prerequisites

- Docker and Docker Compose
- A self-hosted [ntfy.sh](https://ntfy.sh) instance (or the public one)
- A PostgreSQL-compatible setup (included via Docker Compose)
- [Nix](https://nixos.org/) for building the Docker image and CLI

## 1. Build the Docker image and CLI

```bash
nix build .#docker
docker load < result

nix build .#cli
# CLI binary is at ./result/bin/oubot-cli
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

./result/bin/oubot-cli --server "$OUBOT_SERVER" init
```

This prints your admin access token. Save it:

```bash
export OUBOT_TOKEN=<printed-token>
```

Verify:

```bash
./result/bin/oubot-cli me
```

## 6. Create a regular user

Generate an invite token, then use it to create a user:

```bash
# As admin: create an invite
./result/bin/oubot-cli admin create-invite
# Prints: Invite token: <invite-token>

# Create a new user with that invite
./result/bin/oubot-cli --server "$OUBOT_SERVER" init --invite <invite-token>
# Prints the new user's access token
```

Save the user's access token — this is what goes on the client device.

## 7. Configure and flash the client device

### Pico W (ESP32-compatible MicroPython)

The client firmware is in `clients/pico-w/blink.py`. Edit it with your configuration:

```python
host = "your-server-domain.com"    # Your server's hostname
token = "token tk_abc123..."       # The user's access token (with "token " prefix)
ssid = "your-wifi-name"            # 2.4 GHz WiFi network name
password = "your-wifi-password"    # WiFi password
```

Flash the device:

```bash
# 1. Plug Pico W in with BOOTSEL button held down
# 2. Flash MicroPython firmware
sudo picotool load clients/pico-w/RPI_PICO_W-20241025-v1.24.0.uf2

# 3. Unplug and replug WITHOUT holding the button

# 4. Deploy the script to the device
sudo rshell -p /dev/ttyACM0 --buffer-size 512 \
  cp clients/pico-w/blink.py /pyboard/main.py

# 5. Unplug and replug — the device starts pinging automatically
```

The device will connect to WiFi and ping `GET /api/v1/up` every ~5 seconds. If pings stop for longer than the user's `up_delay` (default 30s), the server sends a "power off" notification via ntfy.sh. When pings resume, it sends a "power on" notification with the duration of the outage.

## 8. Manage notifications

```bash
# Check ntfy settings
./result/bin/oubot-cli ntfy show

# Disable/enable notifications
./result/bin/oubot-cli ntfy disable
./result/bin/oubot-cli ntfy enable

# Change notification language (uk or en)
./result/bin/oubot-cli language en
```

## 9. Regenerate tokens

If a token is compromised:

```bash
./result/bin/oubot-cli token regenerate
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
