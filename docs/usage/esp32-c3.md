# ESP32-C3 Client — Usage Guide

Guide for invite token holders setting up an ESP32-C3 uptime monitoring device.

## Prerequisites

- **ESP32-C3 board** (any dev board with USB-C, e.g. ESP32-C3-DevKitM-1)
- **USB-C cable** for flashing
- **WiFi credentials** for a 2.4GHz network (ESP32-C3 doesn't support 5GHz)
- **Invite token** from your server admin
- **Nix** with flakes enabled (provides the full toolchain)

All commands below use `nix develop -c` or `nix build` and run from the **repository root**.

## 1. Get Your Access Token

Ask your server admin for an invite token, then register:

```bash
export OUBOT_SERVER=https://oubot.example.com

# Register with your invite token:
nix develop -c oubot-cli init --invite <your-invite-token>
# Outputs: Your access token: tk_...
```

Save the access token — you'll need it for the firmware build.

The first user on a fresh server doesn't need an invite token (becomes admin automatically).

## 2. Build the Firmware

All configuration is baked into the firmware at compile time via environment variables:

```bash
OUBOT_WIFI_SSID="YourWiFiName" \
OUBOT_WIFI_PASS="YourWiFiPassword" \
OUBOT_SERVER="https://oubot.example.com" \
OUBOT_TOKEN="tk_your_access_token" \
nix build .#esp32-client --impure
```

| Variable | Description |
|----------|-------------|
| `OUBOT_WIFI_SSID` | 2.4GHz WiFi network name |
| `OUBOT_WIFI_PASS` | WiFi password |
| `OUBOT_SERVER` | Server URL (e.g. `https://oubot.example.com`) |
| `OUBOT_TOKEN` | Access token from step 1 (e.g. `tk_abc123...`) |

## 3. Flash the Device

Connect the ESP32-C3 via USB-C, then:

```bash
nix develop -c espflash flash --monitor result/bin/esp32-uptime-client
```

You should see:

```
INFO - WiFi connected!
INFO - Got IP: 192.168.x.x/24
INFO - Connection established
INFO - up: ok
INFO - up: ok
```

The LED blinks briefly on each successful heartbeat.

To flash without the monitor: `nix develop -c espflash flash result/bin/esp32-uptime-client`

### Alternative: cargo build + flash (for development)

If you're iterating on the firmware code, the devShell cargo workflow is faster (from `clients/esp32`):

```bash
cd clients/esp32

OUBOT_WIFI_SSID="YourWiFiName" \
OUBOT_WIFI_PASS="YourWiFiPassword" \
OUBOT_SERVER="https://oubot.example.com" \
OUBOT_TOKEN="tk_your_access_token" \
nix develop -c cargo run --release
```

This builds, flashes, and opens the serial monitor in one step.

## 4. Verify It Works

```bash
export OUBOT_SERVER=https://oubot.example.com
export OUBOT_TOKEN=tk_your_access_token
nix develop -c oubot-cli me
```

The uptime state should show as "up". Unplug the device for 60+ seconds and you'll receive a "down" notification via ntfy.

## 5. LED Behavior

| Pattern | Meaning |
|---------|---------|
| Brief blink every ~7s | Heartbeat sent successfully |
| Solid on | Error state (bad status or connection failure), retrying with backoff |
| Off | Idle / sleeping between heartbeats |

## 6. Token Regeneration

If you regenerate your access token, the device will start getting 401 errors. Rebuild and re-flash with the new token (from the repository root):

```bash
OUBOT_WIFI_SSID="YourWiFiName" \
OUBOT_WIFI_PASS="YourWiFiPassword" \
OUBOT_SERVER="https://oubot.example.com" \
OUBOT_TOKEN="tk_new_token" \
nix build .#esp32-client --impure

nix develop -c espflash flash --monitor result/bin/esp32-uptime-client
```

## 7. Erase Device

To wipe the ESP32-C3 before giving it to someone else (removes compiled-in WiFi credentials and access token):

```bash
nix develop -c espflash erase-flash
```

This erases the entire flash. The device will be blank — it needs new firmware flashed to function again.

## 8. Troubleshooting

**WiFi won't connect:**
- Verify the SSID is a 2.4GHz network (ESP32-C3 doesn't support 5GHz)
- Check the password — special characters work but must be passed correctly via the env var
- If using WPA3-only, try switching the router to WPA2/WPA3 mixed mode

**Device keeps resetting:**
- Open the serial monitor (`nix develop -c espflash monitor`) to see the panic message
- Most common cause: incorrect `opt-level` (must be "s" or higher for WiFi to work)

**Getting 401 errors:**
- Token might be expired or regenerated — rebuild with the current token
- Check that `OUBOT_TOKEN` includes the `tk_` prefix

**No LED blink:**
- GPIO8 might be a WS2812 RGB LED on some boards (won't respond to simple GPIO toggle)
- Try connecting an external LED to GPIO2 and changing the pin in `main.rs`

**espflash can't find the device:**
- Check `ls /dev/ttyACM*` — the ESP32-C3's USB JTAG/serial appears as `/dev/ttyACM0`
- Your user needs to be in the `dialout` group (`sudo usermod -aG dialout $USER`, then re-login)
- Try holding BOOT, pressing RESET, then releasing BOOT to enter download mode
