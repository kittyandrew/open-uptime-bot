# Security test: validates auth failure logging, IP rate limiting, metrics,
# and fail2ban banning.
#
# @NOTE: Two-node test. The server runs oubot + fail2ban, the client sends
#  requests. fail2ban bans the client's IP (192.168.1.1) after 5 failures.
#  We verify detection, banning, and that banned requests are dropped.
(import ./lib/lib.nix) {
  name = "security-auth";

  nodes = {
    server = {
      pkgs,
      oubot,
      tester-script,
      ...
    }: let
      c = import ./lib/config.nix;
    in {
      imports = [(import ./lib/services.nix {inherit pkgs oubot;})];

      environment.systemPackages = [oubot tester-script pkgs.curl pkgs.jq];
      environment.variables = {
        OUBOT_BASE_URL = "http://${c.host}:${c.oubot-port}";
        NTFY_BASE_URL = "${c.host}:${c.ntfy-port}";
      };

      # Open all ports so the client VM can reach oubot
      networking.firewall.allowedTCPPortRanges = [{ from = 1; to = 65535; }];

      # @NOTE: Same pattern as kittyos hosts/tustan/default.nix mailu-smtp-auth jail.
      #  Inline filter + journalmatch, systemd backend. maxretry=5 triggers actual ban.
      services.fail2ban = {
        enable = true;
        maxretry = 5;
        bantime = "10m";
        bantime-increment = {
          enable = true;
          maxtime = "168h";
        };
        jails.oubot-auth = {
          filter = {
            Definition.failregex = "^.* \\[AUTH\\] ip=<HOST> result=(invalid_token|missing_header)";
            Init.journalmatch = "_SYSTEMD_UNIT=open-uptime-bot.service";
          };
          settings = {
            enabled = true;
            backend = "systemd";
            maxretry = 5;
            findtime = "5m";
            bantime = "1h";
            # @NOTE: In production (kittyos), this would be:
            #  action = iptables-multiport[name=oubot, port="80,443", chain=DOCKER-USER]
            #  Here we use the default iptables action which adds to the INPUT chain.
          };
        };
      };
    };

    client = {pkgs, ...}: {
      environment.systemPackages = [pkgs.curl pkgs.jq];
    };
  };

  testScript = let
    c = import ./lib/config.nix;
  in ''
    import time
    import re
    import json

    server.wait_for_unit("open-uptime-bot")
    server.wait_for_open_port(${c.oubot-port})
    server.wait_for_unit("fail2ban")

    # Wait for the server to be reachable from the client
    client.wait_until_succeeds(
        "curl -sf http://server:${c.oubot-port}/api/v1/health",
        timeout=60
    )

    # --- Success path: create admin user from client ---
    response = client.succeed(
        "curl -s -X POST http://server:${c.oubot-port}/api/v1/users "
        "-H 'Content-Type: application/json' "
        "-d '{\"user_type\": \"Admin\", \"invites_limit\": 5, \"up_delay\": 5, \"ntfy_enabled\": true, \"language_code\": \"en\"}'"
    )
    data = json.loads(response)
    print(f"Create user response: {data}")
    assert data["status"] == 200, f"Expected status 200, got: {response}"

    # --- Verify active_users metric (from server localhost, unaffected by client ban) ---
    server.succeed(
        "curl -sf http://localhost:${c.oubot-port}/api/v1/metrics "
        "| grep -q '^oubot_active_users 1'"
    )

    # --- Fire exactly 5 invalid auth requests to trigger fail2ban (maxretry=5) ---
    # Use sleep between requests to avoid IP rate limiter (5 req/sec) eating them.
    for i in range(5):
        client.succeed(
            "curl -s "
            "-H 'Authorization: token tk_invalid_token_test' "
            "http://server:${c.oubot-port}/api/v1/up"
        )
        time.sleep(0.3)

    # --- Verify auth failure logs in journald ---
    server.succeed("journalctl -u open-uptime-bot --no-pager | grep -q '\\[AUTH\\].*result=invalid_token'")

    # --- Verify auth failure metrics incremented ---
    server.wait_until_succeeds(
        "curl -sf http://localhost:${c.oubot-port}/api/v1/metrics "
        "| grep -q 'oubot_auth_failures_total.*invalid_token'",
        timeout=10
    )

    # --- Wait for fail2ban to process and ban ---
    server.wait_until_succeeds(
        "fail2ban-client status oubot-auth | grep -qP 'Currently banned:\\s+[1-9]'",
        timeout=30
    )
    result = server.succeed("fail2ban-client status oubot-auth")
    print(f"fail2ban status: {result}")

    # Assert systemd backend
    assert "Journal matches" in result, f"Expected systemd journal backend, got: {result}"

    # Assert fail2ban counted failures
    total_match = re.search(r"Total failed:\s+(\d+)", result)
    assert total_match, f"Could not parse Total failed from: {result}"
    total_failed = int(total_match.group(1))
    assert total_failed >= 5, f"Expected fail2ban to detect >= 5 failures, got {total_failed}"
    print(f"fail2ban detected {total_failed} failures - regex works!")

    # Assert the client IP was actually banned
    banned_match = re.search(r"Currently banned:\s+(\d+)", result)
    assert banned_match, f"Could not parse Currently banned from: {result}"
    currently_banned = int(banned_match.group(1))
    assert currently_banned >= 1, f"Expected >= 1 banned IP, got {currently_banned}"
    print(f"fail2ban banned {currently_banned} IP(s)!")

    # --- Verify banned client can no longer reach the server ---
    exit_code, _ = client.execute(
        "curl -sf --connect-timeout 5 http://server:${c.oubot-port}/api/v1/health"
    )
    assert exit_code != 0, "Expected banned client to be unable to reach server"
    print("Banned client correctly rejected!")

    # --- Test missing header auth failure (from server localhost, unaffected by ban) ---
    server.succeed(
        "curl -s -o /dev/null http://localhost:${c.oubot-port}/api/v1/up"
    )
    server.succeed("journalctl -u open-uptime-bot --no-pager | grep -q '\\[AUTH\\].*result=missing_header'")

    # --- Verify IP rate limiter by firing a burst from localhost ---
    # @NOTE: Fire 10 rapid requests (no sleep) to exceed 5 req/sec limit.
    #  Then check that rate_limited appears in metrics.
    for i in range(10):
        server.execute(
            "curl -s "
            "-H 'Authorization: token tk_invalid_burst_test' "
            "http://localhost:${c.oubot-port}/api/v1/up"
        )
    time.sleep(1)
    server.wait_until_succeeds(
        "curl -sf http://localhost:${c.oubot-port}/api/v1/metrics "
        "| grep -q 'oubot_auth_failures_total.*rate_limited'",
        timeout=10
    )
    print("IP rate limiter verified!")

    # --- Verify all expected metrics exist ---
    metrics = server.succeed(
        "curl -sf http://localhost:${c.oubot-port}/api/v1/metrics"
    )
    assert "oubot_auth_failures_total" in metrics, "Expected auth failure metrics"
    assert "oubot_requests_total" in metrics, "Expected request total metric"
    assert "oubot_active_users 1" in metrics, "Expected active_users = 1"
    print("All security assertions passed!")
  '';
}
