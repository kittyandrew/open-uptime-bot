use crate::actions::{self, NewInvite, NewUser};
use crate::{DB, context::Context};
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

#[get("/api/v1/users/<uid>")]
pub async fn get_user(uid: uuid::Uuid, context: &State<Context>) -> Value {
    match context.users.read().await.get(&uid).clone() {
        Some(state) => json!({"status": 200, "state": state}),
        None => json!({"status": 400, "error": "User not found!"}),
    }
}

#[post("/api/v1/invites", data = "<opts>")]
pub async fn create_invite(opts: Json<NewInvite>, mut conn: Connection<DB>, context: &State<Context>) -> Value {
    match actions::create_invite(&opts, &mut conn, context).await {
        Ok(invite) => json!({"status": 200, "invite": invite}),
        Err(err) => json!({"status": 400, "error": err}),
    }
}
