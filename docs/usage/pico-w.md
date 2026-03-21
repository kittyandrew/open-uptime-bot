# Pico W Client — Usage Guide

Guide for invite token holders setting up a Raspberry Pi Pico W uptime monitoring device.

## Prerequisites

- **Raspberry Pi Pico W** (RP2040 + CYW43439 WiFi)
- **Micro-USB cable** for flashing
- **WiFi credentials** for a 2.4GHz WPA2 network
- **Invite token** from your server admin
- **Nix** with flakes enabled (provides the full toolchain)

All commands below run from the **repository root**.

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
nix build .#pico-w-client --impure
```

| Variable | Description |
|----------|-------------|
| `OUBOT_WIFI_SSID` | 2.4GHz WiFi network name |
| `OUBOT_WIFI_PASS` | WiFi password |
| `OUBOT_SERVER` | Server URL (e.g. `https://oubot.example.com`) |
| `OUBOT_TOKEN` | Access token from step 1 (e.g. `tk_abc123...`) |

## 3. Flash the Device

picotool can flash even when the Pico W is running existing firmware — BOOTSEL mode is not required:

```bash
nix develop -c picotool load -x result/bin/pico-w-uptime-client -t elf
```

The `-t elf` flag tells picotool the file type (the binary has no extension). The `-x` flag reboots the device after loading.

If picotool reports "no accessible RP-series devices", hold BOOTSEL while plugging in the USB cable, then retry.

### Alternative: elf2uf2-rs

Convert to UF2 and copy to the Pico W's USB mass storage (requires BOOTSEL mode):

```bash
nix develop -c elf2uf2-rs result/bin/pico-w-uptime-client result/pico-w-uptime-client.uf2
# Copy the .uf2 file to the Pico W drive (appears when holding BOOTSEL during plug-in)
```

### Alternative: cargo build + flash (for development)

If you're iterating on the firmware code, the devShell cargo workflow is faster (from `clients/pico-w`):

```bash
cd clients/pico-w

OUBOT_WIFI_SSID="YourWiFiName" \
OUBOT_WIFI_PASS="YourWiFiPassword" \
OUBOT_SERVER="https://oubot.example.com" \
OUBOT_TOKEN="tk_your_access_token" \
nix develop -c cargo run --release
```

This builds and flashes via `picotool load` (configured as the cargo runner in `.cargo/config.toml`). The device reboots automatically after flashing.

### Monitoring output

The Pico W uses defmt/RTT for logging (not serial output). To see logs, use a debug probe (e.g. another Pico as picoprobe) with `probe-rs`. Without a probe, verify heartbeats via the server metrics or CLI instead.

## 4. Verify It Works

```bash
export OUBOT_SERVER=https://oubot.example.com
export OUBOT_TOKEN=tk_your_access_token
nix develop -c oubot-cli me
```

The uptime state should show as "up". You can also check the Prometheus metrics endpoint:

```bash
curl $OUBOT_SERVER/api/v1/metrics | grep oubot_uptime
```

Unplug the device for 30+ seconds and you'll receive a "down" notification via ntfy.

## 5. LED Behavior

| Pattern | Meaning |
|---------|---------|
| Brief blink every ~5s | Heartbeat sent successfully |
| Solid on | Error state or halted (bad token / connection failure) |
| Off | Idle / sleeping between heartbeats |

The Pico W's onboard LED is controlled through the CYW43439 WiFi chip (active-high), not a regular GPIO.

## 6. Token Regeneration

If you regenerate your access token, the device will halt after 5 consecutive 401 errors. Rebuild and re-flash with the new token:

```bash
OUBOT_WIFI_SSID="YourWiFiName" \
OUBOT_WIFI_PASS="YourWiFiPassword" \
OUBOT_SERVER="https://oubot.example.com" \
OUBOT_TOKEN="tk_new_token" \
nix build .#pico-w-client --impure

nix develop -c picotool load -x result/bin/pico-w-uptime-client -t elf
```

## 7. Troubleshooting

**WiFi won't connect:**
- Verify the SSID is a 2.4GHz network (CYW43439 doesn't support 5GHz)
- The Pico W defaults to WPA2/WPA3 — ensure your router supports one of these

**picotool can't find the device:**
- Try holding BOOTSEL while plugging in the USB cable to enter bootloader mode
- Requires udev rules for the RP2040 USB device (`idVendor=2e8a`). On NixOS, add `services.udev.packages = [ pkgs.picotool ];` or equivalent rules with your user's group
- Try `sudo picotool info` to verify the device is detected but lacks permissions

**No LED activity:**
- The LED requires the WiFi driver to be initialized (it's on the CYW43439 chip)
- If the firmware panics before WiFi init, the LED won't turn on — use a debug probe to check
- Without a probe, check server metrics to see if heartbeats are arriving
