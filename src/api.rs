use crate::actions::{self, NewInvite, NewUser};
use crate::{DB, bauth, context::Context, db};
use rocket::State;
use rocket::serde::json::{Json, Value, json};
use rocket_db_pools::Connection;

#[post("/api/v1/users", data = "<opts>")]
pub async fn create_user(opts: Json<NewUser>, mut conn: Connection<DB>, context: &State<Context>) -> Value {
    match actions::create_user(&opts, &mut conn, context).await {
        Ok(state) => json!({"status": 200, "state": state}),
        Err(err) => json!({"status": 400, "error": err}),
    }
}

/// Create a new invite (admin only)
#[post("/api/v1/invites")]
pub async fn create_invite(admin: bauth::AdminAuth, mut conn: Connection<DB>, context: &State<Context>) -> Value {
    let opts = NewInvite { owner_id: admin.uid };
    match actions::create_invite(&opts, &mut conn, context).await {
        Ok(invite) => json!({"status": 200, "invite": invite}),
        Err(err) => json!({"status": 400, "error": err}),
    }
}

/// List all invites (admin only)
#[get("/api/v1/invites")]
pub async fn list_invites(admin: bauth::AdminAuth, mut conn: Connection<DB>) -> Value {
    match db::get_invites_for_user(&mut conn, admin.uid).await {
        Ok(invites) => json!({"status": 200, "invites": invites}),
        Err(err) => json!({"status": 500, "error": format!("{err:?}")}),
    }
}

/// Delete an invite (admin only)
#[delete("/api/v1/invites/<invite_id>")]
pub async fn delete_invite(
    admin: bauth::AdminAuth,
    invite_id: uuid::Uuid,
    mut conn: Connection<DB>,
    context: &State<Context>,
) -> Value {
    match db::delete_invite(&mut conn, invite_id, admin.uid).await {
        Ok(deleted) if deleted > 0 => {
            let was_unused = context.remove_invite(invite_id).await;
            if was_unused {
                if let Some(state) = context.users.write().await.get_mut(&admin.uid) {
                    state.user.invites_used -= 1;
                }
            }
            json!({"status": 200, "message": "Invite deleted"})
        }
        Ok(_) => json!({"status": 404, "error": "Invite not found or not owned by you"}),
        Err(err) => json!({"status": 500, "error": format!("{err:?}")}),
    }
}

/// Delete a user (admin only)
#[delete("/api/v1/admin/users/<user_id>")]
pub async fn delete_user(
    admin: bauth::AdminAuth,
    user_id: uuid::Uuid,
    mut conn: Connection<DB>,
    context: &State<Context>,
) -> Value {
    if admin.uid == user_id {
        return json!({"status": 400, "error": "Cannot delete yourself"});
    }
    // Collect data needed for cleanup before deletion (DB will cascade-delete invites)
    let invite_ids: Vec<uuid::Uuid> = match db::get_invites_for_user(&mut conn, user_id).await {
        Ok(invites) => invites.iter().filter(|i| !i.is_used).map(|i| i.id).collect(),
        Err(err) => {
            warn!("Failed to load invites for user {user_id} during deletion: {err:?}");
            vec![]
        }
    };
    let ntfy_username = context.users.read().await
        .get(&user_id).map(|s| s.ntfy.username.clone());
    match db::delete_user(&mut conn, user_id).await {
        Ok(deleted) if deleted > 0 => {
            context.remove_user(user_id).await;
            context.remove_invite_ids(&invite_ids).await;
            // Clean up ntfy.sh server user
            if let Some(username) = ntfy_username {
                if let Err(err) = context.ntfy.delete_user(&username).await {
                    warn!("Failed to delete ntfy user '{username}': {err:?}");
                }
            }
            json!({"status": 200, "message": "User deleted"})
        }
        Ok(_) => json!({"status": 404, "error": "User not found"}),
        Err(err) => json!({"status": 500, "error": format!("{err:?}")}),
    }
}

// User self-service endpoints

/// Get current authenticated user info
#[get("/api/v1/me")]
pub async fn get_me(bauth: bauth::BAuth, context: &State<Context>) -> Value {
    match context.users.read().await.get(&bauth.uid) {
        Some(state) => json!({"status": 200, "user": state}),
        None => json!({"status": 404, "error": "User not found"}),
    }
}

/// Regenerate access token (for Pico W client)
#[post("/api/v1/me/regenerate-token")]
pub async fn regenerate_token(
    bauth: bauth::BAuth,
    mut conn: Connection<DB>,
    context: &State<Context>,
) -> Value {
    match db::regenerate_user_token(&mut conn, bauth.uid).await {
        Ok(new_token) => {
            // Update in-memory state
            let old_token = {
                let mut users = context.users.write().await;
                if let Some(state) = users.get_mut(&bauth.uid) {
                    let old = state.user.access_token.clone();
                    state.user.access_token = new_token.clone();
                    old
                } else {
                    return json!({"status": 404, "error": "User not found"});
                }
            };
            // Update tokens map
            {
                let mut tokens = context.tokens.write().await;
                tokens.remove(&old_token);
                tokens.insert(new_token.clone(), bauth.uid);
            }
            json!({"status": 200, "access_token": new_token})
        }
        Err(e) => json!({"status": 500, "error": format!("{:?}", e)}),
    }
}

/// Get ntfy notification settings
#[get("/api/v1/me/ntfy")]
pub async fn get_ntfy_settings(bauth: bauth::BAuth, context: &State<Context>) -> Value {
    match context.users.read().await.get(&bauth.uid) {
        Some(state) => json!({
            "status": 200,
            "ntfy": {
                "enabled": state.ntfy.enabled,
                "topic": state.ntfy.topic,
                "username": state.ntfy.username,
                "password": state.ntfy.password,
            }
        }),
        None => json!({"status": 404, "error": "User not found"}),
    }
}

/// Update ntfy notification settings (enable/disable)
#[derive(rocket::serde::Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct UpdateNtfy {
    pub enabled: bool,
}

#[patch("/api/v1/me/ntfy", data = "<opts>")]
pub async fn update_ntfy_settings(
    bauth: bauth::BAuth,
    opts: Json<UpdateNtfy>,
    mut conn: Connection<DB>,
    context: &State<Context>,
) -> Value {
    let ntfy_id = match context.users.read().await.get(&bauth.uid) {
        Some(state) => state.ntfy.id,
        None => return json!({"status": 404, "error": "User not found"}),
    };

    match db::update_ntfy_enabled(&mut conn, ntfy_id, opts.enabled).await {
        Ok(_) => {
            // Update in-memory state
            if let Some(state) = context.users.write().await.get_mut(&bauth.uid) {
                state.ntfy.enabled = opts.enabled;
            }
            json!({"status": 200, "enabled": opts.enabled})
        }
        Err(e) => json!({"status": 500, "error": format!("{:?}", e)}),
    }
}

/// Get current language setting
#[get("/api/v1/me/language")]
pub async fn get_language(bauth: bauth::BAuth, context: &State<Context>) -> Value {
    match context.users.read().await.get(&bauth.uid) {
        Some(state) => json!({
            "status": 200,
            "language_code": state.user.language_code
        }),
        None => json!({"status": 404, "error": "User not found"}),
    }
}

/// Update language setting
#[derive(rocket::serde::Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct UpdateLanguage {
    pub language_code: String,
}

#[patch("/api/v1/me/language", data = "<opts>")]
pub async fn update_language(
    bauth: bauth::BAuth,
    opts: Json<UpdateLanguage>,
    mut conn: Connection<DB>,
    context: &State<Context>,
) -> Value {
    let lang = &opts.language_code;
    if let Err(err) = actions::validate_language_code(lang) {
        return json!({"status": 400, "error": err});
    }

    match db::update_user_language(&mut conn, bauth.uid, lang).await {
        Ok(_) => {
            // Update in-memory state
            if let Some(state) = context.users.write().await.get_mut(&bauth.uid) {
                state.user.language_code = lang.clone();
            }
            json!({"status": 200, "language_code": lang})
        }
        Err(e) => json!({"status": 500, "error": format!("{:?}", e)}),
    }
}

// Admin endpoints

/// List all users (admin only).
/// @NOTE: Intentionally exposes full user data including access_token and ntfy
///  credentials — admins have explicit access to manage and impersonate any user.
#[get("/api/v1/admin/users")]
pub async fn admin_list_users(_admin: bauth::AdminAuth, context: &State<Context>) -> Value {
    let users: Vec<db::UserState> = context.users.read().await.values().cloned().collect();
    json!({"status": 200, "users": users})
}

/// Get any user by ID (admin only)
#[get("/api/v1/admin/users/<uid>")]
pub async fn admin_get_user(_admin: bauth::AdminAuth, uid: uuid::Uuid, context: &State<Context>) -> Value {
    match context.users.read().await.get(&uid) {
        Some(state) => json!({"status": 200, "user": state}),
        None => json!({"status": 404, "error": "User not found"}),
    }
}

