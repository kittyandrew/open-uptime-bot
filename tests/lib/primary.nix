{
  pkgs,
  oubot,
  tester-script,
  ...
}: let
  c = import ./config.nix;
in {
  environment.systemPackages = [oubot tester-script];
  environment.variables = {
    OUBOT_BASE_URL = "http://${c.host}:${c.oubot-port}";
    NTFY_BASE_URL = "${c.host}:${c.ntfy-port}";
  };

  services = {
    postgresql = {
      enable = true;
      settings = {
        port = pkgs.lib.strings.toInt c.psql-port;
      };
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
        # auth-file = "${./ntfy-user.db}"; # This is too painful to do otherwise.
        auth-default-access = "deny-all";
        # Not actually needed here, but hey lets reproduce my setup.
        behind-proxy = true;
        enable-metrics = true;
      };
    };
  };

  systemd.services = {
    # @nocheckin: document/explain
    open-uptime-bot = {
      enable = true;
      wantedBy = ["multi-user.target"];
      requires = ["postgresql.service" "ntfy-sh.service"];
      after = ["postgresql.service" "ntfy-sh.service"];
      script = ''
        # @nocheckin: document (once to not repeat in readme?)
        set -e

        # Run ntfy preparation.
        NTFY_PASSWORD=${c.pass} ${pkgs.ntfy-sh}/bin/ntfy user add --role=admin ${c.user}
        raw_token_out=$(${pkgs.ntfy-sh}/bin/ntfy token add ${c.user} 2>&1)
        echo 2>&1 $raw_token_out  # We still want to put output into logs.
        export NTFY_ADMIN_TOKEN=$(echo $raw_token_out | cut -d " " -f2)
        echo 2>&1 "Exporting admin access token: '$NTFY_ADMIN_TOKEN'"

        # Values below are the defaults I use on my instance.
        # The "human-readable" name is different from "tier code"
        # and is unimportant for all our intents and purposes.
        ${pkgs.ntfy-sh}/bin/ntfy tier add \
          --name="basic" \
          --message-limit=1000 \
          --message-expiry-duration=24h \
          --reservation-limit=0 \
          --attachment-file-size-limit=100M \
          --attachment-total-size-limit=1G \
          --attachment-expiry-duration=12h \
          --attachment-bandwidth-limit=5G \
          ${c.ntfy-tier}

        ${oubot}/bin/oubot
      '';
      environment = {
        LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath [pkgs.openssl];
        NTFY_BASE_URL = "http://${c.host}:${c.ntfy-port}";
        NTFY_USER_TIER = c.ntfy-tier;
        DATABASE_URL = "postgres://${c.psql-user}:a@localhost:${c.psql-port}/${c.psql-db}";
      };
    };
  };
}
