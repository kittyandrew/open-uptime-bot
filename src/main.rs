#[macro_use]
extern crate rocket;

use dotenv;
use fluent_templates::static_loader;
use grammers_client::Update;
use grammers_client::types::chat;
use grammers_tl_types::{enums, types};
use prometheus::TextEncoder;
use rocket::fairing::AdHoc;
use rocket::http::Status;
use rocket::serde::json::{Value, json};
use rocket::{State, tokio};
use rocket_db_pools::diesel::PgPool;
use rocket_db_pools::{Connection, Database};
use std::env::var;
use std::time::{Duration, SystemTime};

mod actions;
mod api;
mod bauth;
mod context;
mod db;
mod ntfy;
mod prom;
mod schema;
mod tg;

static_loader! {
    static LOCALES = {
        locales: "./locales",
        fallback_language: "en",
        // @NOTE: I don't know what this does exactly, but we have to disable
        //  it because it breaks grammers parsing of the embedded links.
        customise: |bundle| bundle.set_use_isolating(false),
    };
}

#[derive(Database)]
#[database("open-uptime-bot")]
pub struct DB(PgPool);

// @nocheckin: strings here should be from locales.
async fn dispatch_notifications(item: db::UserState, context: context::Context) {
    if item.ntfy.enabled {
        let notification = ntfy::NtfyNotification {
            topic: item.ntfy.topic.clone(),
            title: match item.uptime.status {
                // @nocheckin: this notification has to be sent otherwise its bad UX.
                db::UpStatus::Uninitialized => "Девайс під'єднано!".to_string(),
                db::UpStatus::Up => "Світло з'явилося!".to_string(),
                db::UpStatus::Down => "Відключення світла!".to_string(),
                db::UpStatus::Maintainance => "Service is going on maintance!".to_string(),
            },
            message: "(тут буде цікава інформація про графік/тривалість відключень)".to_string(),
            status: match item.uptime.status {
                db::UpStatus::Uninitialized => "white_check_mark".to_string(),
                db::UpStatus::Up => "white_check_mark".to_string(),
                db::UpStatus::Down => "warning".to_string(),
                db::UpStatus::Maintainance => "warning".to_string(),
            },
            priority: "high".to_string(), // Configurable for user.
        };

        tokio::spawn(async move {
            println!("Sending ntfy {notification:?} to {u:?}", u = item.ntfy.username);
            if let Err(err) = context.ntfy.send_notification(notification).await {
                println!("Failed attempting to send ntfy-cation: {err:?}");
            };
        });
    }

    if let Some(tg) = context.tg {
        if item.tg.enabled {
            let message = match item.uptime.status {
                db::UpStatus::Uninitialized => "Девайс під'єднано!",
                db::UpStatus::Up => "Світло з'явилося!",
                db::UpStatus::Down => "Відключення світла!",
                db::UpStatus::Maintainance => "Service is going on maintance!",
            };

            tokio::spawn(async move {
                let user = chat::User::from_raw(enums::User::Empty(types::UserEmpty { id: item.tg.user_id }));
                println!("Sending tg {message:?} to {user:?}");
                if let Err(err) = tg.send_message(&user, message).await {
                    println!("Failed when attempting to send message: {err}");
                };
            });
        }
    } else if item.tg.enabled {
        warn!("TG notifications are enabled for the user, but TG client is missing!");
    }
}

async fn background_handle_down(context: context::Context) {
    loop {
        let mut sleep_for = Duration::new(5, 0);
        let mut user_ids = Vec::new();
        {
            for (_, r_value) in context.users.read().await.iter() {
                let query_at = r_value.uptime.touched_at + Duration::new(r_value.user.up_delay as u64, 0);
                if let Ok(item) = query_at.duration_since(SystemTime::now()) {
                    sleep_for = item;
                } else {
                    user_ids.push(r_value.user.id);
                }
            }
        }
        {
            let mut guard = context.users.write().await;
            for user_id in user_ids {
                let item = guard.get_mut(&user_id).unwrap();
                if item.uptime.go_down() {
                    // We get into the 'else' statement if the returned time is negative,
                    // which in other words means that we are past the threshold time and
                    // notifications need to be fired off.
                    tokio::spawn(dispatch_notifications(item.clone(), context.clone()));
                }
            }
        }
        // @NOTE: This is the default sleep, which handles a case where all clients went
        //  offline, which would be pretty rare at scale, but we must handle this case
        //  anyways, since in case that happens we don't want to run without sleep.
        tokio::time::sleep(sleep_for).await;
    }
}

async fn background_handle_telegram(context: context::Context, db_pool: PgPool) {
    loop {
        match context.tg.as_ref().unwrap().next_update().await {
            Ok(update) => match update {
                Update::NewMessage(message) => {
                    // Ignoring bot's own message.
                    if !message.outgoing() {
                        tokio::spawn(tg::handle_new_message(message, context.clone(), db_pool.clone()));
                    }
                }
                Update::CallbackQuery(callback) => {
                    tokio::spawn(tg::handle_new_callback(callback, context.clone(), db_pool.clone()));
                }
                _ => {}
            },
            Err(err) => {
                println!("Error fetching updates: {err}. Retrying in a bit...");
                tokio::time::sleep(Duration::new(10, 0)).await;
            }
        };
    }
}

#[get("/api/v1/health")]
async fn api_health(mut conn: Connection<DB>) -> Value {
    // TODO: proper healthcheck.
    return json!({"status": 200});
}

#[get("/api/v1/up")]
async fn api_up(bauth: bauth::BAuth, context: &State<context::Context>) -> Status {
    let mut guard = context.users.write().await;
    let item = guard.get_mut(&bauth.uid).expect("RIP");
    // This will automatically update ('touch') the 'query_at' value and push
    // it back by the defined threshold. Additionally, it returns bool indicating
    // whether state was changed from something to "up", so we will fire off
    // notifications if it returns true.
    if item.uptime.touch() {
        tokio::spawn(dispatch_notifications(item.clone(), context.inner().clone()));
    };
    return Status::Ok;
}

#[rocket::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();

    // Since there is a hack used to lazy load those constants, we want to reset them here,
    // so they are immediately usable from start (they will be recognized by the prometheus
    // gatherer and encoder, as opposed to not shown until first increment).
    prom::TOTAL_REQUESTS_SERVED.reset();
    prom::ENDPOINTS_REQUESTS_SERVED.reset();

    let figment = rocket::Config::figment().merge(("databases.open-uptime-bot", rocket_db_pools::Config {
        url: var("DATABASE_URL").expect("Database URL required"),
        min_connections: Some(4),
        max_connections: 1024,
        connect_timeout: 5,
        idle_timeout: None,
        extensions: None,
    }));

    rocket::custom(figment)
        .mount("/", routes![
            prom::get_metrics,
            api::create_user,
            api::get_user,
            api::create_invite,
            api_up,
            api_health,
        ])
        .manage(context::Context::init().await)
        .attach(DB::init())
        // Encoder for the prometheus metadata.
        .manage(TextEncoder::new())
        .manage(bauth::get_rate_limiter())
        .attach(prom::PrometheusCollection)
        .attach(AdHoc::try_on_ignite("init db load", |rocket| async {
            // Populating users/tokens from the database.
            let mut conn = DB::fetch(&rocket).unwrap().0.clone().get().await.unwrap();
            let items = db::get_all_states(&mut conn).await.unwrap();
            info!("Loading {n} users from the database!", n = items.len());

            let context = rocket.state::<context::Context>().unwrap();
            for state in items {
                context.add_state(state).await;
            }
            Ok(rocket)
        }))
        .attach(AdHoc::try_on_ignite("background handle down", |rocket| async {
            let context = rocket.state::<context::Context>().unwrap();
            tokio::spawn(background_handle_down(context.clone()));
            Ok(rocket)
        }))
        .attach(AdHoc::try_on_ignite("background handle telegram", |rocket| async {
            let context = rocket.state::<context::Context>().unwrap();
            if context.tg.is_some() {
                let pool = DB::fetch(&rocket).expect("RIP").0.clone();
                tokio::spawn(background_handle_telegram(context.clone(), pool));
            } else {
                warn!("TG client missing so background TG listener not initiated!");
            }
            Ok(rocket)
        }))
        .ignite()
        .await?
        .launch()
        .await?;

    Ok(())
}
