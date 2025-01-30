use crate::context::Context;
use crate::db::{self, Invite, User, UserState};
use rocket::serde::Deserialize;
use rocket_db_pools::diesel::AsyncPgConnection as Conn;

#[derive(Debug, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct NewUser {
    pub invite: Option<String>,
    pub user_type: db::UserType,
    pub invites_limit: i64,
    pub up_delay: Option<u16>,
    pub down_delay: Option<u16>,
    pub ntfy_enabled: bool,
    pub tg_enabled: bool,
    pub tg_user_id: i64,
    pub tg_language_code: String,
}

// @NOTE: This does not check for stuff like same telegram user who is already authenticated
//  using invite and somehow making a new user. That particular case has to be checked on the
//  telegram UI/adapter level.                                          - andrew, Nov 10 2024
pub async fn create_user(opts: &NewUser, conn: &mut Conn, context: &Context) -> Result<UserState, String> {
    // @nocheckin: this acts as lock per token?
    let tokens;
    let mut token_id = None;
    if let Some(new_invite) = &opts.invite {
        tokens = context.tokens.write().await;
        token_id = match tokens.get(new_invite) {
            Some(invite_id) => Some(invite_id),
            None => return Err("Provided invite token does not exist!".to_string()),
        }
    }

    let ntfy = match context.ntfy.create_new_user(opts.ntfy_enabled).await {
        Ok(new_ntfy_user) => new_ntfy_user,
        Err(err) => return Err(format!("{err:?}")),
    };
    let tg = db::TelegramUser::new(opts.tg_enabled, opts.tg_user_id, opts.tg_language_code.clone());
    let new_user = User::new(
        opts.user_type,
        opts.invites_limit,
        opts.up_delay,
        opts.down_delay,
        &ntfy,
        &tg,
    );
    let new_state = db::UserState {
        uptime: db::UptimeState::new(new_user.id),
        user: new_user,
        ntfy,
        tg,
    };

    if let Err(err) = db::create_new_state(conn, &new_state, token_id).await {
        return Err(format!("{err:?}"));
    };
    context.add_state(new_state.clone()).await;
    Ok(new_state)
}

#[derive(Debug, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct NewInvite {
    pub owner_id: db::ID,
}

pub async fn create_invite(opts: &NewInvite, conn: &mut Conn, context: &Context) -> Result<Invite, String> {
    {
        let users = context.users.read().await;
        match users.get(&opts.owner_id) {
            Some(state) if state.user.invites_used >= state.user.invites_limit => {
                return Err("Invites used matching invites limit, early exiting!".to_string());
            }
            None => return Err("User was not found for given 'owner_id'!".to_string()),
            _ => {}
        };
    }

    let new_invite = db::Invite::new(opts.owner_id.clone());
    match db::create_new_invite(conn, &new_invite).await {
        Ok(_) => {
            context.add_invite(new_invite.clone()).await;
            Ok(new_invite)
        }
        Err(err) => Err(format!("DB Err: {err:?}")),
    }
}

#[derive(Debug, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct NewUserViaInvite {
    pub user: NewUser,
    pub invite: String,
}

/*
pub async fn create_user_via_invite(
    opts: &NewUserViaInvite,
    conn: &mut Conn,
    context: &Context,
) -> Result<UserState, String> {
}
*/
