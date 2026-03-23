# CLI package — standard x86_64 Rust build with OpenSSL wrapper.
# Called from flake.nix with the server's crane lib and pkgs.
#
# @NOTE: oubot-cli links against OpenSSL dynamically (via reqwest).
# makeWrapper sets LD_LIBRARY_PATH so libssl.so is found at runtime.
{
  craneLib,
  pkgs,
}: let
  raw = craneLib.buildPackage {
    src = ./.;
    nativeBuildInputs = [pkgs.pkg-config];
    buildInputs = [pkgs.openssl];
  };
in
  pkgs.runCommand "oubot-cli-wrapped" {
    nativeBuildInputs = [pkgs.makeWrapper];
  } ''
    mkdir -p $out/bin
    makeWrapper ${raw}/bin/oubot-cli $out/bin/oubot-cli \
      --prefix LD_LIBRARY_PATH : ${pkgs.lib.makeLibraryPath [pkgs.openssl]}
  ''
