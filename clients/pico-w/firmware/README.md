# CYW43439 WiFi Firmware Blobs

Binary firmware files for the CYW43439 WiFi chip on the Raspberry Pi Pico W. The chip has no onboard flash — these are loaded over SPI on every boot.

## Files

| File | Size | Purpose |
|------|------|---------|
| `43439A0.bin` | ~231KB | Main WiFi firmware (radio, MAC, PHY) |
| `43439A0_clm.bin` | ~984B | Country Locale Matrix — regulatory TX power/channel tables |
| `nvram_rp2040.bin` | ~742B | Board-specific config (antenna, clocks, GPIO mapping for RP2040) |

## Source

Downloaded from the [embassy-rs/embassy](https://github.com/embassy-rs/embassy) repository, directory [`cyw43-firmware/`](https://github.com/embassy-rs/embassy/tree/main/cyw43-firmware).

These are Cypress/Infineon proprietary blobs redistributed under the **Permissive Binary License 1.0** (see `LICENSE`).

## Updating

These blobs are updated very rarely — they're tied to the CYW43439 chip revision (`43439A0`), not to software releases. To update:

1. Check the [embassy cyw43-firmware directory](https://github.com/embassy-rs/embassy/tree/main/cyw43-firmware) for newer files
2. Download and replace the three `.bin` files in this directory
3. Rebuild and test the firmware (`nix build .#pico-w-client --impure` or `cargo run --release`)
4. Verify WiFi still connects and heartbeats flow

There is no versioning on the blobs themselves — compare file sizes or checksums to detect changes.

## Usage in Code

The files are loaded at compile time via `include_bytes!` / `cyw43::aligned_bytes!` in `src/main.rs`:

```rust
let fw = cyw43::aligned_bytes!("../firmware/43439A0.bin");
let nvram = cyw43::aligned_bytes!("../firmware/nvram_rp2040.bin");
let clm = include_bytes!("../firmware/43439A0_clm.bin");
```

They're baked into the final ELF binary, adding ~232KB to the firmware image.
