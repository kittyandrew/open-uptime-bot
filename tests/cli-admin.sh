#!/usr/bin/env bash
#
# CLI Admin Test
#
# Tests admin-specific commands:
# 1. Admin users (list)
# 2. Admin user <id> (get specific)
# 3. Admin invites (list)
# 4. Admin delete-invite
#

set -euo pipefail

SERVER="http://0.0.0.0:8000"

echo "============================================================"
echo "CLI Admin Test"
echo "============================================================"

# Setup: Initialize admin and create a user
echo ""
echo "[Setup] Initialize admin"
INIT_OUTPUT=$(oubot-cli --server "$SERVER" init)
ADMIN_TOKEN=$(echo "$INIT_OUTPUT" | grep "Your access token:" | awk '{print $4}')
if [ -z "$ADMIN_TOKEN" ]; then
    echo "ERROR: Failed to extract admin token"
    exit 1
fi
echo "Admin ready"

echo ""
echo "[Setup] Create invite and user"
INVITE_OUTPUT=$(oubot-cli --server "$SERVER" --token "$ADMIN_TOKEN" admin create-invite)
INVITE_TOKEN=$(echo "$INVITE_OUTPUT" | grep "Invite token:" | awk '{print $3}')
USER_INIT_OUTPUT=$(oubot-cli --server "$SERVER" init --invite "$INVITE_TOKEN")
USER_TOKEN=$(echo "$USER_INIT_OUTPUT" | grep "Your access token:" | awk '{print $4}')
if [ -z "$USER_TOKEN" ]; then
    echo "ERROR: Failed to create user"
    exit 1
fi

# Get user ID
ME_OUTPUT=$(oubot-cli --server "$SERVER" --token "$USER_TOKEN" me)
USER_ID=$(echo "$ME_OUTPUT" | grep "^ID:" | awk '{print $2}')
echo "User ID: $USER_ID"
echo "Setup complete (1 admin + 1 user)"

sleep 1

# Step 1: Admin users (list all)
echo ""
echo "[Step 1] Admin users list"
USERS_OUTPUT=$(oubot-cli --server "$SERVER" --token "$ADMIN_TOKEN" admin users)
echo "$USERS_OUTPUT"
if ! echo "$USERS_OUTPUT" | grep -q "Total: 2 user(s)"; then
    echo "ERROR: Should show 2 users"
    exit 1
fi
# Verify table has both Admin and Normal
if ! echo "$USERS_OUTPUT" | grep -q "Admin"; then
    echo "ERROR: Should show Admin user type"
    exit 1
fi
if ! echo "$USERS_OUTPUT" | grep -q "Normal"; then
    echo "ERROR: Should show Normal user type"
    exit 1
fi
echo "Users list correct"

sleep 1

# Step 2: Admin user <id> (get specific user)
echo ""
echo "[Step 2] Admin get specific user"
USER_OUTPUT=$(oubot-cli --server "$SERVER" --token "$ADMIN_TOKEN" admin user "$USER_ID")
echo "$USER_OUTPUT"
if ! echo "$USER_OUTPUT" | grep -q "ID:.*$USER_ID"; then
    echo "ERROR: Should show the requested user's ID"
    exit 1
fi
if ! echo "$USER_OUTPUT" | grep -q "Type:.*Normal"; then
    echo "ERROR: Should show user type Normal"
    exit 1
fi
echo "Specific user lookup correct"

sleep 1

# Step 3: Admin invites (list)
echo ""
echo "[Step 3] Admin invites list"
INVITES_OUTPUT=$(oubot-cli --server "$SERVER" --token "$ADMIN_TOKEN" admin invites)
echo "$INVITES_OUTPUT"
# We created 1 invite and used it, so there should be 1 invite total
if ! echo "$INVITES_OUTPUT" | grep -q "Total: 1 invite(s)"; then
    echo "ERROR: Should show 1 invite"
    exit 1
fi
echo "Invites list correct"

sleep 1

# Step 4: Create a second (unused) invite and delete it
echo ""
echo "[Step 4] Create second invite"
INVITE2_OUTPUT=$(oubot-cli --server "$SERVER" --token "$ADMIN_TOKEN" --raw admin create-invite)
echo "$INVITE2_OUTPUT"
# Extract first UUID (invite id) from JSON output
INVITE2_ID=$(echo "$INVITE2_OUTPUT" | grep -o '"id": "[^"]*"' | head -1 | cut -d'"' -f4)
if [ -z "$INVITE2_ID" ]; then
    echo "ERROR: Failed to extract invite ID"
    exit 1
fi
echo "Second invite ID: $INVITE2_ID"

sleep 1

# Verify now 2 invites
echo ""
echo "[Step 4b] Verify 2 invites"
INVITES_OUTPUT=$(oubot-cli --server "$SERVER" --token "$ADMIN_TOKEN" admin invites)
echo "$INVITES_OUTPUT"
if ! echo "$INVITES_OUTPUT" | grep -q "Total: 2 invite(s)"; then
    echo "ERROR: Should show 2 invites"
    exit 1
fi
echo "2 invites confirmed"

sleep 1

# Delete the unused invite
echo ""
echo "[Step 4c] Delete unused invite"
oubot-cli --server "$SERVER" --token "$ADMIN_TOKEN" admin delete-invite "$INVITE2_ID"
echo "Invite deleted"

sleep 1

# Verify back to 1 invite
echo ""
echo "[Step 4d] Verify 1 invite after deletion"
INVITES_OUTPUT=$(oubot-cli --server "$SERVER" --token "$ADMIN_TOKEN" admin invites)
echo "$INVITES_OUTPUT"
if ! echo "$INVITES_OUTPUT" | grep -q "Total: 1 invite(s)"; then
    echo "ERROR: Should show 1 invite after deletion"
    exit 1
fi
echo "Invite deletion verified"

sleep 1

# Cleanup: delete user
echo ""
echo "[Cleanup] Delete user"
oubot-cli --server "$SERVER" --token "$ADMIN_TOKEN" admin delete-user "$USER_ID"
echo "User deleted"

echo ""
echo "============================================================"
echo "All admin tests passed!"
echo "============================================================"
