# Shared service definitions for NixOS integration tests.
# Imports infra.nix (PostgreSQL + ntfy-sh) and adds the oubot systemd service.
# Tests that need only the infrastructure (e.g., docker-e2e) import infra.nix directly.
{
  pkgs,
  oubot,
  ...
}: let
  c = import ./config.nix;
in {
  imports = [(import ./infra.nix {inherit pkgs;})];

  systemd.services.open-uptime-bot = {
    enable = true;
    wantedBy = ["multi-user.target"];
    requires = ["postgresql.service" "ntfy-sh.service"];
    after = ["postgresql.service" "ntfy-sh.service"];
    script = ''
      set -e
      ${import ./ntfy-bootstrap.nix {inherit pkgs;}}
      export NTFY_ADMIN_TOKEN
      ${oubot}/bin/oubot
    '';
    environment = {
      LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath [pkgs.openssl];
      NTFY_BASE_URL = "http://${c.host}:${c.ntfy-port}";
      NTFY_USER_TIER = c.ntfy-tier;
      DATABASE_URL = "postgres://${c.psql-user}:a@localhost:${c.psql-port}/${c.psql-db}";
      # @NOTE: Without Rocket.toml (which lives in the repo, not the Nix store),
      #  Rocket defaults to 127.0.0.1. Bind to 0.0.0.0 so multi-node tests can
      #  reach the server from other VMs.
      ROCKET_ADDRESS = "0.0.0.0";
    };
  };
}
