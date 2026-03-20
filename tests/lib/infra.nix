# Shared infrastructure services for NixOS integration tests.
# PostgreSQL + ntfy-sh — used by both native oubot tests (via services.nix)
# and Docker E2E tests (directly).
{
  pkgs,
  ...
}: let
  c = import ./config.nix;
in {
  services = {
    postgresql = {
      enable = true;
      settings.port = pkgs.lib.strings.toInt c.psql-port;
      # @NOTE: Allowing unauthenticated connection by anyone from anyone
      #  for any database, which is perfectly fine in our test setup.
      authentication = pkgs.lib.mkForce ''
        local all all              trust
        host  all all 127.0.0.1/32 trust
        host  all all ::1/128      trust
      '';
    };

    ntfy-sh = {
      enable = true;
      settings = {
        base-url = "http://${c.host}";
        listen-http = ":${c.ntfy-port}";
        auth-default-access = "deny-all";
        # Not actually needed here, but hey lets reproduce my setup.
        behind-proxy = true;
        enable-metrics = true;
      };
    };
  };
}
