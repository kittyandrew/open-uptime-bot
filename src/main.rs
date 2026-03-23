#[macro_use]
extern crate rocket;

use prometheus::TextEncoder;
use rocket::fairing::AdHoc;
use rocket::tokio;
use rocket_db_pools::Database;
use rocket_db_pools::diesel::PgPool;
use std::env::var;

mod actions;
mod api;
mod background;
mod bauth;
mod context;
mod db;
mod notifications;
mod ntfy;
mod prom;
mod schema;

#[derive(Database)]
#[database("open-uptime-bot")]
pub struct DB(PgPool);

#[rocket::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();

    // Since there is a hack used to lazy load those constants, we want to reset them here,
    // so they are immediately usable from start (they will be recognized by the prometheus
    // gatherer and encoder, as opposed to not shown until first increment).
    prom::TOTAL_REQUESTS_SERVED.reset();
    prom::ENDPOINTS_REQUESTS_SERVED.reset();
    prom::ACTIVE_USERS.set(0);

    let figment = rocket::Config::figment().merge((
        "databases.open-uptime-bot",
        rocket_db_pools::Config {
            url: var("DATABASE_URL").expect("Database URL required"),
            min_connections: Some(4),
            max_connections: 1024,
            connect_timeout: 5,
            idle_timeout: None,
            extensions: None,
        },
    ));

    // @WARNING: Every route handler MUST use BAuth, AdminAuth, or RateLimitGuard
    //  to ensure IP rate limiting coverage. The IpRateLimitFairing sets a flag but
    //  can't reject requests in Rocket 0.5 — guards must check the flag.
    //  The route-guard-lint check in flake.nix enforces this at build time.
    rocket::custom(figment)
        .mount(
            "/",
            routes![
                prom::get_metrics,
                api::create_user,
                api::create_invite,
                api::list_invites,
                api::delete_invite,
                api::get_me,
                api::regenerate_token,
                api::get_ntfy_settings,
                api::update_ntfy_settings,
                api::get_language,
                api::update_language,
                api::pause_monitoring,
                api::unpause_monitoring,
                api::get_settings,
                api::update_settings,
                api::admin_list_users,
                api::admin_get_user,
                api::delete_user,
                api::api_up,
                api::api_health,
            ],
        )
        .manage(context::Context::init())
        .attach(DB::init())
        // Encoder for the prometheus metadata.
        .manage(TextEncoder::new())
        .manage(bauth::get_rate_limiter())
        .attach(bauth::IpRateLimitFairing)
        .attach(prom::PrometheusCollection)
        .attach(AdHoc::try_on_ignite("init db load", |rocket| async {
            // Populating users/tokens from the database.
            let mut conn = DB::fetch(&rocket).unwrap().0.clone().get().await.unwrap();
            let items = db::get_all_states(&mut conn).await.unwrap();
            info!("Loading {n} users from the database!", n = items.len());

            let context = rocket.state::<context::Context>().unwrap();
            for state in &items {
                // Initialize per-user metrics from DB state
                let uid_str = state.user.id.to_string();
                prom::UPTIME_STATE
                    .with_label_values(&[&uid_str])
                    .set(i64::from(&state.uptime.status));
                let touched_ts = state
                    .uptime
                    .touched_at
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs_f64();
                prom::LAST_SEEN_TIMESTAMP.with_label_values(&[&uid_str]).set(touched_ts);
            }
            prom::ACTIVE_USERS.set(items.len() as i64);
            for state in items {
                context.add_state(state).await;
            }

            // Load unused invites into memory
            let invites = db::get_all_unused_invites(&mut conn).await.unwrap();
            info!("Loading {n} invites from the database!", n = invites.len());
            for invite in invites {
                context.invite_tokens.write().await.insert(invite.token, invite.id);
            }

            Ok(rocket)
        }))
        .attach(AdHoc::try_on_ignite("background handle down", |rocket| async {
            let context = rocket.state::<context::Context>().unwrap();
            let pool = DB::fetch(&rocket).expect("RIP").0.clone();
            tokio::spawn(background::background_handle_down(context.clone(), pool));
            Ok(rocket)
        }))
        .ignite()
        .await?
        .launch()
        .await?;

    Ok(())
}
