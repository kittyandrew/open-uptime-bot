use crate::db::{ID, Invite, UserState};
use crate::{ntfy::NtfyClient, tg};
use rocket::tokio::sync::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct Context {
    pub users: Arc<RwLock<HashMap<ID, UserState>>>,
    pub tokens: Arc<RwLock<HashMap<String, ID>>>,
    pub tg_users: Arc<RwLock<HashMap<i64, ID>>>,
    pub invite_tokens: Arc<RwLock<HashMap<String, ID>>>,
    pub ntfy: NtfyClient,
    pub tg: Option<grammers_client::Client>,
}

impl Context {
    pub async fn init() -> Context {
        let tg = match tg::init_client().await {
            Ok(tg_client) => Some(tg_client),
            Err(err) => {
                warn!("Telegram client was not initiated: {err:?}");
                None
            }
        };

        return Context {
            users: Default::default(),
            tokens: Default::default(),
            tg_users: Default::default(),
            invite_tokens: Default::default(),
            ntfy: NtfyClient::new(),
            tg,
        };
    }

    pub async fn add_state(&self, v: UserState) {
        self.tokens.write().await.insert(v.user.access_token.clone(), v.user.id);
        self.tg_users.write().await.insert(v.tg.user_id, v.user.id);

        if let Some(old) = self.users.write().await.insert(v.user.id, v) {
            warn!("Creating new user state, but one already existed: {old:?}");
        }
    }

    pub async fn add_invite(&self, v: Invite) {
        self.invite_tokens.write().await.insert(v.token, v.id);

        let mut users = self.users.write().await;
        let state = users.get_mut(&v.owner_id).unwrap();
        state.user.invites_used += 1;
    }
}
