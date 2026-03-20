#!/usr/bin/env bash
#
# CLI Settings Test
#
# Tests user self-service settings commands:
# 1. Token show
# 2. Language change + verify
# 3. Ntfy disable/show/enable/show cycle
# 4. Token regenerate + verify old fails + new works
#

set -euo pipefail

SERVER="${OUBOT_BASE_URL:?OUBOT_BASE_URL must be set}"

echo "============================================================"
echo "CLI Settings Test"
echo "============================================================"

# Setup: Initialize admin account
echo ""
echo "[Setup] Initialize admin account"
INIT_OUTPUT=$(oubot-cli --server "$SERVER" init)
ADMIN_TOKEN=$(echo "$INIT_OUTPUT" | grep "Your access token:" | awk '{print $4}')
if [ -z "$ADMIN_TOKEN" ]; then
    echo "ERROR: Failed to extract admin token"
    exit 1
fi
echo "Admin token ready"

# Step 1: Token show
echo ""
echo "[Step 1] Token show"
TOKEN_OUTPUT=$(oubot-cli --server "$SERVER" --token "$ADMIN_TOKEN" token show)
echo "Token show output: $TOKEN_OUTPUT"
# Verify the shown token matches what we have
if [ "$TOKEN_OUTPUT" != "$ADMIN_TOKEN" ]; then
    echo "ERROR: Token show output '$TOKEN_OUTPUT' doesn't match admin token '$ADMIN_TOKEN'"
    exit 1
fi
echo "Token show matches"

# Step 2: Language change
echo ""
echo "[Step 2] Change language to 'en'"
LANG_OUTPUT=$(oubot-cli --server "$SERVER" --token "$ADMIN_TOKEN" language en)
echo "$LANG_OUTPUT"
if ! echo "$LANG_OUTPUT" | grep -q "Language set to: en"; then
    echo "ERROR: Expected 'Language set to: en'"
    exit 1
fi

sleep 1

# Verify language changed via 'me'
echo ""
echo "[Step 2b] Verify language changed"
ME_OUTPUT=$(oubot-cli --server "$SERVER" --token "$ADMIN_TOKEN" me)
echo "$ME_OUTPUT"
if ! echo "$ME_OUTPUT" | grep -q "Language:.*en"; then
    echo "ERROR: Language should show 'en' after change"
    exit 1
fi
echo "Language correctly changed"

# Step 3: Ntfy disable/enable cycle
echo ""
echo "[Step 3] Ntfy disable"
oubot-cli --server "$SERVER" --token "$ADMIN_TOKEN" ntfy disable

sleep 1

echo ""
echo "[Step 3b] Verify ntfy disabled"
NTFY_OUTPUT=$(oubot-cli --server "$SERVER" --token "$ADMIN_TOKEN" ntfy show)
echo "$NTFY_OUTPUT"
if ! echo "$NTFY_OUTPUT" | grep -q "\[OFF\]"; then
    echo "ERROR: Ntfy should show as disabled [OFF]"
    exit 1
fi
echo "Ntfy correctly disabled"

sleep 1

echo ""
echo "[Step 3c] Ntfy enable"
oubot-cli --server "$SERVER" --token "$ADMIN_TOKEN" ntfy enable

sleep 1

echo ""
echo "[Step 3d] Verify ntfy enabled"
NTFY_OUTPUT=$(oubot-cli --server "$SERVER" --token "$ADMIN_TOKEN" ntfy show)
echo "$NTFY_OUTPUT"
if ! echo "$NTFY_OUTPUT" | grep -q "\[ON\]"; then
    echo "ERROR: Ntfy should show as enabled [ON]"
    exit 1
fi
# Verify ntfy show displays all expected fields
if ! echo "$NTFY_OUTPUT" | grep -q "Topic:"; then
    echo "ERROR: Ntfy show should display Topic"
    exit 1
fi
if ! echo "$NTFY_OUTPUT" | grep -q "Username:"; then
    echo "ERROR: Ntfy show should display Username"
    exit 1
fi
if ! echo "$NTFY_OUTPUT" | grep -q "Password:"; then
    echo "ERROR: Ntfy show should display Password"
    exit 1
fi
echo "Ntfy correctly enabled and all fields present"

# Step 4: Token regenerate
echo ""
echo "[Step 4] Token regenerate"
REGEN_OUTPUT=$(oubot-cli --server "$SERVER" --token "$ADMIN_TOKEN" --raw token regenerate)
echo "$REGEN_OUTPUT"
# Token always starts with tk_ followed by alphanumeric chars
NEW_TOKEN=$(echo "$REGEN_OUTPUT" | grep -o 'tk_[A-Za-z0-9]*')
if [ -z "$NEW_TOKEN" ]; then
    echo "ERROR: Failed to extract new token from regenerate response"
    exit 1
fi
echo "New token: $NEW_TOKEN"

sleep 1

# Verify old token no longer works
echo ""
echo "[Step 4b] Verify old token rejected"
if oubot-cli --server "$SERVER" --token "$ADMIN_TOKEN" me 2>/dev/null; then
    echo "ERROR: Old token should have been rejected after regeneration"
    exit 1
fi
echo "Old token correctly rejected"

# Verify new token works
echo ""
echo "[Step 4c] Verify new token works"
ME_OUTPUT=$(oubot-cli --server "$SERVER" --token "$NEW_TOKEN" me)
echo "$ME_OUTPUT"
if ! echo "$ME_OUTPUT" | grep -q "User Info"; then
    echo "ERROR: New token should work"
    exit 1
fi
echo "New token works correctly"

echo ""
echo "============================================================"
echo "All settings tests passed!"
echo "============================================================"
