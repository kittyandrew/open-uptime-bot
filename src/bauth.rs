use crate::{context, db};
use governor::{Quota, RateLimiter, clock::QuantaClock, state::InMemoryState};
use rocket::State;
use rocket::http::Status;
use rocket::request::{FromRequest, Outcome, Request};
use std::num::NonZero;
use std::sync::Arc;
use uuid::Uuid;

type Limiter = Arc<RateLimiter<Uuid, dashmap::DashMap<Uuid, InMemoryState>, QuantaClock>>;

pub fn get_rate_limiter() -> Limiter {
    // @TODO: make rate limit configurable (per user?)
    Arc::new(RateLimiter::keyed(Quota::per_second(NonZero::new(2u32).unwrap())))
}

#[derive(Debug)]
pub struct BAuth {
    pub uid: db::ID,
}

#[derive(Debug)]
pub struct AdminAuth {
    pub uid: db::ID,
}

#[derive(Debug)]
pub enum BAuthError {
    Missing,
    Invalid,
    RateLimited,
    NotAdmin,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for BAuth {
    type Error = BAuthError;

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        match req.headers().get_one("Authorization") {
            Some(raw) => {
                let tokens = &req.guard::<&State<context::Context>>().await.unwrap().tokens;
                match tokens.read().await.get(raw.strip_prefix("token ").unwrap_or(raw)) {
                    Some(uid) => match req.guard::<&State<Limiter>>().await.unwrap().check_key(&uid) {
                        Ok(_) => Outcome::Success(BAuth { uid: *uid }),
                        Err(_) => Outcome::Error((Status::TooManyRequests, BAuthError::RateLimited)),
                    },
                    None => Outcome::Error((Status::Unauthorized, BAuthError::Invalid)),
                }
            }
            None => Outcome::Error((Status::Unauthorized, BAuthError::Missing)),
        }
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for AdminAuth {
    type Error = BAuthError;

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        match req.headers().get_one("Authorization") {
            Some(raw) => {
                let context = &req.guard::<&State<context::Context>>().await.unwrap();
                let tokens = &context.tokens;
                match tokens.read().await.get(raw.strip_prefix("token ").unwrap_or(raw)) {
                    Some(uid) => {
                        // Check rate limit
                        match req.guard::<&State<Limiter>>().await.unwrap().check_key(&uid) {
                            Ok(_) => {
                                // Check if user is admin
                                let users = context.users.read().await;
                                match users.get(uid) {
                                    Some(user_state) if user_state.user.user_type == db::UserType::Admin => {
                                        Outcome::Success(AdminAuth { uid: *uid })
                                    }
                                    Some(_) => Outcome::Error((Status::Forbidden, BAuthError::NotAdmin)),
                                    None => Outcome::Error((Status::Unauthorized, BAuthError::Invalid)),
                                }
                            }
                            Err(_) => Outcome::Error((Status::TooManyRequests, BAuthError::RateLimited)),
                        }
                    }
                    None => Outcome::Error((Status::Unauthorized, BAuthError::Invalid)),
                }
            }
            None => Outcome::Error((Status::Unauthorized, BAuthError::Missing)),
        }
    }
}
