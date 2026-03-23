# Docker E2E test: validates the Nix-built Docker image works end-to-end.
# Runs oubot from the Docker container (same as production) with PostgreSQL
# and ntfy.sh as native NixOS services (imported from infra.nix).
# Pattern adapted from github.com/kittyandrew/grafana-to-ntfy docker test.
(import ./lib/lib.nix) {
  name = "docker-e2e";

  nodes = {
    primary = {
      pkgs,
      oubot,
      tester-script,
      docker-image,
      ...
    }: let
      c = import ./lib/config.nix;
    in {
      imports = [(import ./lib/infra.nix {inherit pkgs;})];

      environment.systemPackages = [oubot tester-script pkgs.curl pkgs.jq];
      environment.variables = {
        OUBOT_BASE_URL = "http://${c.host}:${c.oubot-port}";
        NTFY_BASE_URL = "${c.host}:${c.ntfy-port}";
      };

      virtualisation.docker.enable = true;

      # @NOTE: Prepare ntfy admin user + tier, then start oubot Docker container.
      #  Uses --network host so the container can reach PostgreSQL and ntfy on localhost.
      systemd.services.oubot-docker = {
        after = ["docker.service" "postgresql.service" "ntfy-sh.service"];
        requires = ["docker.service" "postgresql.service" "ntfy-sh.service"];
        wantedBy = ["multi-user.target"];
        serviceConfig = {
          Type = "oneshot";
          RemainAfterExit = true;
        };
        path = [pkgs.docker];
        script = ''
          set -e
          ${import ./lib/ntfy-bootstrap.nix {inherit pkgs;}}

          # Load and run Docker image
          docker load < ${docker-image}
          docker run -d \
            --name oubot \
            --network host \
            -e "DATABASE_URL=postgres://${c.psql-user}:a@localhost:${c.psql-port}/${c.psql-db}" \
            -e "NTFY_BASE_URL=http://${c.host}:${c.ntfy-port}" \
            -e "NTFY_ADMIN_TOKEN=$NTFY_ADMIN_TOKEN" \
            -e "NTFY_USER_TIER=${c.ntfy-tier}" \
            -e "ROCKET_PORT=${c.oubot-port}" \
            open-uptime-bot:${docker-image.imageTag}
        '';
      };
    };
  };

  testScript = let
    c = import ./lib/config.nix;
  in ''
    primary.wait_for_unit("docker.service")
    primary.wait_for_unit("ntfy-sh")
    primary.wait_for_open_port(${c.ntfy-port})
    primary.wait_for_unit("oubot-docker")

    import time
    import json
    primary.wait_until_succeeds(
        "curl -sf http://localhost:${c.oubot-port}/api/v1/health",
        timeout=60
    )

    # --- Create admin user and extract token ---
    response = primary.succeed(
        "curl -sf -X POST http://localhost:${c.oubot-port}/api/v1/users "
        "-H 'Content-Type: application/json' "
        "-d '{\"user_type\": \"Admin\", \"invites_limit\": 5, \"up_delay\": 10, \"ntfy_enabled\": true, \"language_code\": \"en\"}'"
    )
    data = json.loads(response)
    assert data["status"] == 200, f"Expected status 200, got: {response}"
    token = data["state"]["user"]["access_token"]
    print(f"Admin token: {token}")

    # --- Verify authenticated endpoint works ---
    primary.succeed(
        f"curl -sf -H 'Authorization: token {token}' "
        f"http://localhost:${c.oubot-port}/api/v1/me | jq -e '.status == 200'"
    )

    # --- Send heartbeat and verify uptime state metric ---
    primary.succeed(
        f"curl -sf -H 'Authorization: token {token}' "
        f"http://localhost:${c.oubot-port}/api/v1/up"
    )
    time.sleep(1)
    primary.wait_until_succeeds(
        "curl -sf http://localhost:${c.oubot-port}/api/v1/metrics "
        "| grep -q 'oubot_uptime_state.*1'",
        timeout=10
    )

    # --- Verify health endpoint ---
    primary.succeed(
        "curl -sf http://localhost:${c.oubot-port}/api/v1/health | jq -e '.status == 200'"
    )

    # --- Verify metrics endpoint (including notification from heartbeat) ---
    # @NOTE: Generous timeouts because the Docker-in-VM environment adds latency
    #  to the async notification chain (tokio::spawn → HTTP to ntfy-sh → metric).
    primary.wait_until_succeeds(
        "curl -sf http://localhost:${c.oubot-port}/api/v1/metrics | grep -q 'oubot_active_users'",
        timeout=30
    )
    # @NOTE: Check for notification metric. If absent after timeout, dump container
    #  logs so we can diagnose whether the ntfy send failed or never ran.
    try:
        primary.wait_until_succeeds(
            "curl -sf http://localhost:${c.oubot-port}/api/v1/metrics "
            "| grep -q 'oubot_notifications_total'",
            timeout=30
        )
    except Exception:
        print("=== oubot container logs ===")
        print(primary.succeed("docker logs oubot 2>&1 || true"))
        print("=== metrics dump ===")
        print(primary.succeed("curl -sf http://localhost:${c.oubot-port}/api/v1/metrics || true"))
        raise

    # Verify it was a successful notification, not a failure
    metrics = primary.succeed(
        "curl -sf http://localhost:${c.oubot-port}/api/v1/metrics"
    )
    assert "oubot_notifications_total" in metrics, (
        f"Notification metric missing entirely. Container logs:\\n"
        f"{primary.succeed('docker logs oubot 2>&1 || true')}"
    )
    assert 'type="connected"' in metrics and 'result="success"' in metrics, (
        f"Expected connected/success notification, got:\\n"
        f"{[l for l in metrics.splitlines() if 'notification' in l]}"
    )

    # --- Verify Docker container is running ---
    primary.succeed("docker ps | grep -q oubot")
  '';
}
