# ESP32-C3 cross-compilation package.
# Called from flake.nix with the ESP32-specific crane lib.
{craneLib}: let
  # @NOTE: Requires `nix build --impure` with all 4 env vars set.
  # builtins.getEnv returns "" in pure mode; const assertions in main.rs
  # reject empty values at compile time.
  envVars = {
    OUBOT_WIFI_SSID = builtins.getEnv "OUBOT_WIFI_SSID";
    OUBOT_WIFI_PASS = builtins.getEnv "OUBOT_WIFI_PASS";
    OUBOT_SERVER = builtins.getEnv "OUBOT_SERVER";
    OUBOT_TOKEN = builtins.getEnv "OUBOT_TOKEN";
  };

  commonArgs = envVars // {
    pname = "esp32-uptime-client";
    version = "2026.3.23";
    src = ./.;
    doCheck = false; # Can't run no_std binary on build host.
    cargoExtraArgs = "--target riscv32imc-unknown-none-elf";
    # @NOTE: Strip build-std from .cargo/config.toml.
    # Nix build uses pre-built rust-std from fenix; build-std is only
    # for devShell where the complete.toolchain has rust-src.
    postUnpack = ''
      sed -i '/^\[unstable\]$/d; /^build-std/d' $sourceRoot/.cargo/config.toml
    '';
  };
in
  craneLib.buildPackage (commonArgs // {
    cargoArtifacts = craneLib.buildDepsOnly commonArgs;
  })
