# Shared ntfy bootstrap script fragment for integration tests.
# Creates admin user, extracts token, adds default tier.
# After eval, NTFY_ADMIN_TOKEN is set as a shell variable.
{pkgs, ...}: let
  c = import ./config.nix;
  ntfy = "${pkgs.ntfy-sh}/bin/ntfy";
in ''
  NTFY_PASSWORD=${c.pass} ${ntfy} user add --role=admin ${c.user}
  raw_token_out=$(${ntfy} token add ${c.user} 2>&1)
  echo "$raw_token_out"
  NTFY_ADMIN_TOKEN=$(echo "$raw_token_out" | cut -d " " -f2)
  echo "NTFY_ADMIN_TOKEN='$NTFY_ADMIN_TOKEN'"

  # Values below are the defaults I use on my instance.
  # The "human-readable" name is different from "tier code"
  # and is unimportant for all our intents and purposes.
  ${ntfy} tier add \
    --name="basic" \
    --message-limit=1000 \
    --message-expiry-duration=24h \
    --reservation-limit=0 \
    --attachment-file-size-limit=100M \
    --attachment-total-size-limit=1G \
    --attachment-expiry-duration=12h \
    --attachment-bandwidth-limit=5G \
    ${c.ntfy-tier}
''
