# Pico W (RP2040) cross-compilation package.
# Called from flake.nix with the Pico W-specific crane lib.
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

  # @NOTE: cortex-m-rt's link.x expects DefaultHandler_ and other symbols from
  # the #[embassy_executor::main] macro. Crane's deps build creates a dummy
  # main.rs without these, so linking fails. Use `cargo check` for deps to
  # compile without linking, then build the real binary with full source.
  commonArgs =
    envVars
    // {
      pname = "pico-w-uptime-client";
      version = "2026.3.23";
      src = ./.;
      doCheck = false; # Can't run no_std binary on build host.
      cargoExtraArgs = "--target thumbv6m-none-eabi";
      postUnpack = ''
        sed -i '/^\[unstable\]$/d; /^build-std/d' $sourceRoot/.cargo/config.toml
      '';
      # @NOTE: memory.x must be in the linker search path for cortex-m-rt's link.x.
      CARGO_TARGET_THUMBV6M_NONE_EABI_RUSTFLAGS = "-C link-arg=-L${./.}";
    };
in
  craneLib.buildPackage (commonArgs
    // {
      cargoArtifacts = craneLib.buildDepsOnly (commonArgs
        // {
          buildPhaseCargoCommand = "cargo check --release --target thumbv6m-none-eabi";
        });
    })
