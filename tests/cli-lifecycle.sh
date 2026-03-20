#!/usr/bin/env bash
#
# CLI Lifecycle Test
#
# Tests the core user lifecycle:
# 1. Initialize first admin account
# 2. Verify second admin init fails
# 3. Create an invite
# 4. Create a new user with invite (custom language)
# 5. Verify user info (type, language)
# 6. Admin deletes the user
# 7. Verify user's token no longer works
#

set -euo pipefail

SERVER="${OUBOT_BASE_URL:?OUBOT_BASE_URL must be set}"

echo "============================================================"
echo "CLI Lifecycle Test"
echo "============================================================"

# Step 1: Initialize first admin account
echo ""
echo "[Step 1] Initialize first admin account"
INIT_OUTPUT=$(oubot-cli --server "$SERVER" init)
echo "$INIT_OUTPUT"
ADMIN_TOKEN=$(echo "$INIT_OUTPUT" | grep "Your access token:" | awk '{print $4}')
if [ -z "$ADMIN_TOKEN" ]; then
    echo "ERROR: Failed to extract admin token"
    exit 1
fi
echo "Admin token: $ADMIN_TOKEN"

# Step 2: Verify second admin init fails
echo ""
echo "[Step 2] Verify second admin init fails"
if oubot-cli --server "$SERVER" init 2>/dev/null; then
    echo "ERROR: Second admin init should have failed but succeeded"
    exit 1
fi
echo "Second admin init correctly rejected"

# Step 3: Create an invite using admin credentials
echo ""
echo "[Step 3] Create an invite"
INVITE_OUTPUT=$(oubot-cli --server "$SERVER" --token "$ADMIN_TOKEN" admin create-invite)
echo "$INVITE_OUTPUT"
INVITE_TOKEN=$(echo "$INVITE_OUTPUT" | grep "Invite token:" | awk '{print $3}')
if [ -z "$INVITE_TOKEN" ]; then
    echo "ERROR: Failed to extract invite token"
    exit 1
fi
echo "Invite token: $INVITE_TOKEN"

# Step 4: Create a new user with invite and custom language
echo ""
echo "[Step 4] Create user with invite (language=en)"
USER_INIT_OUTPUT=$(oubot-cli --server "$SERVER" init --invite "$INVITE_TOKEN" --language en)
echo "$USER_INIT_OUTPUT"
USER_TOKEN=$(echo "$USER_INIT_OUTPUT" | grep "Your access token:" | awk '{print $4}')
if [ -z "$USER_TOKEN" ]; then
    echo "ERROR: Failed to extract user token"
    exit 1
fi
# Verify it says "User" not "Admin"
if ! echo "$USER_INIT_OUTPUT" | grep -q "User created successfully"; then
    echo "ERROR: Expected 'User created successfully' message"
    exit 1
fi
echo "User token: $USER_TOKEN"

# Step 5: Verify user info
echo ""
echo "[Step 5] Verify user info"
ME_OUTPUT=$(oubot-cli --server "$SERVER" --token "$USER_TOKEN" me)
echo "$ME_OUTPUT"
USER_ID=$(echo "$ME_OUTPUT" | grep "^ID:" | awk '{print $2}')
if [ -z "$USER_ID" ]; then
    echo "ERROR: Failed to extract user ID"
    exit 1
fi
# Verify type is Normal (invited users are always Normal)
if ! echo "$ME_OUTPUT" | grep -q "Type:.*Normal"; then
    echo "ERROR: Invited user should be type Normal"
    exit 1
fi
# Verify language is "en" (set during init)
if ! echo "$ME_OUTPUT" | grep -q "Language:.*en"; then
    echo "ERROR: Language should be 'en' as set during init"
    exit 1
fi
echo "User ID: $USER_ID"

# Step 6: Admin deletes the user
echo ""
echo "[Step 6] Admin deletes the user"
oubot-cli --server "$SERVER" --token "$ADMIN_TOKEN" admin delete-user "$USER_ID"
echo "User deleted"

# Step 7: Verify user's token no longer works
echo ""
echo "[Step 7] Verify user's token no longer works"
if oubot-cli --server "$SERVER" --token "$USER_TOKEN" me 2>/dev/null; then
    echo "ERROR: User's token should have been rejected after deletion"
    exit 1
fi
echo "User's token correctly rejected after deletion"

# Step 8: Verify Prometheus metrics reflect the operations
echo ""
echo "[Step 8] Verify Prometheus metrics"
sleep 1  # Let IP rate limiter refill after rapid CLI commands
METRICS=$(curl -sf "$SERVER/api/v1/metrics")

# After step 6 (user deleted), active_users should be 1 (admin only)
echo "$METRICS" | grep -q '^oubot_active_users 1' || {
    echo "ERROR: Expected oubot_active_users to be 1"; exit 1
}
echo "oubot_active_users correct"

# Request metrics should be populated
echo "$METRICS" | grep -q '^oubot_requests_total ' || {
    echo "ERROR: Expected oubot_requests_total to exist"; exit 1
}
echo "oubot_requests_total present"

echo ""
echo "============================================================"
echo "All lifecycle tests passed!"
echo "============================================================"
