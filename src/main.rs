#[macro_use]
extern crate rocket;

use dotenv;
use fluent::types::FluentValue;
use fluent_templates::{Loader, static_loader};
use std::collections::HashMap;
use unic_langid::LanguageIdentifier;
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

static_loader! {
    static LOCALES = {
        locales: "./locales",
        fallback_language: "en",
        // @NOTE: Disable Unicode isolating marks in Fluent output.
        customise: |bundle| bundle.set_use_isolating(false),
    };
}

/// Formats a duration with days, hours, and minutes using Fluent locale strings.
/// Each part (days, hours, minutes) is looked up via `duration-days`, `duration-hours`,
/// `duration-minutes` keys in the locale files, which handle pluralization via CLDR rules.
fn format_duration(lang: &LanguageIdentifier, duration: Duration) -> String {
    let total_secs = duration.as_secs();
    let days = total_secs / 86400;
    let hours = (total_secs % 86400) / 3600;
    let minutes = (total_secs % 3600) / 60;

    let mut parts = Vec::new();

    if days > 0 {
        let mut args = HashMap::new();
        args.insert("count".to_string(), FluentValue::from(days as i64));
        parts.push(LOCALES.lookup_with_args(lang, "duration-days", &args));
    }

    if hours > 0 {
        let mut args = HashMap::new();
        args.insert("count".to_string(), FluentValue::from(hours as i64));
        parts.push(LOCALES.lookup_with_args(lang, "duration-hours", &args));
    }

    if minutes > 0 || parts.is_empty() {
        let mut args = HashMap::new();
        args.insert("count".to_string(), FluentValue::from(minutes as i64));
        parts.push(LOCALES.lookup_with_args(lang, "duration-minutes", &args));
    }

    parts.join(" ")
}

#[derive(Database)]
#[database("open-uptime-bot")]
pub struct DB(PgPool);

/// Supported locale codes — must match directories under `locales/`.
const SUPPORTED_LOCALES: &[&str] = &["en", "uk"];

async fn dispatch_notifications(item: db::UserState, context: context::Context, duration: Option<Duration>) {
    let lang: LanguageIdentifier = item.user.language_code.parse().unwrap_or_else(|_| {
        warn!("Invalid language code '{}' for user {}, falling back to 'en'", item.user.language_code, item.user.id);
        "en".parse().unwrap()
    });
    if !SUPPORTED_LOCALES.contains(&lang.language.as_str()) {
        warn!("Unsupported locale '{}' for user {}, notifications will use English fallback", item.user.language_code, item.user.id);
    }

    // Format the duration message based on status
    let duration_message = match (item.uptime.status, duration) {
        (db::UpStatus::Up, Some(d)) => {
            let mut args = HashMap::new();
            args.insert("duration".to_string(), FluentValue::from(format_duration(&lang, d)));
            LOCALES.lookup_with_args(&lang, "duration-power-was-off", &args)
        }
        (db::UpStatus::Down, Some(d)) => {
            let mut args = HashMap::new();
            args.insert("duration".to_string(), FluentValue::from(format_duration(&lang, d)));
            LOCALES.lookup_with_args(&lang, "duration-power-was-on", &args)
        }
        _ => String::new(),
    };

    let title = match item.uptime.status {
        db::UpStatus::Uninitialized => LOCALES.lookup(&lang, "notification-device-connected"),
        db::UpStatus::Up => LOCALES.lookup(&lang, "notification-power-on"),
        db::UpStatus::Down => LOCALES.lookup(&lang, "notification-power-off"),
        db::UpStatus::Paused => LOCALES.lookup(&lang, "notification-maintenance"),
    };

    if item.ntfy.enabled {
        let notification = ntfy::NtfyNotification {
            topic: item.ntfy.topic,
            title,
            message: duration_message,
            status: match item.uptime.status {
                db::UpStatus::Uninitialized => "white_check_mark".to_string(),
                db::UpStatus::Up => "white_check_mark".to_string(),
                db::UpStatus::Down => "warning".to_string(),
                db::UpStatus::Paused => "warning".to_string(),
            },
            priority: "high".to_string(), // Configurable for user.
        };

        tokio::spawn(async move {
            info!("Sending ntfy {notification:?} to {u:?}", u = item.ntfy.username);
            if let Err(err) = context.ntfy.send_notification(notification).await {
                warn!("Failed attempting to send ntfy-cation: {err:?}");
            };
        });
    }
}

async fn background_handle_down(context: context::Context, db_pool: PgPool) {
    loop {
        let mut sleep_for = Duration::new(5, 0);
        let mut states_to_persist = Vec::new();
        {
            // @NOTE: Single write lock to atomically check thresholds and transition
            //  states, preventing TOCTOU race with api_up's touch().
            let mut guard = context.users.write().await;
            let now = SystemTime::now();
            for (_, item) in guard.iter_mut() {
                let query_at = item.uptime.touched_at + Duration::new(item.user.up_delay as u64, 0);
                if let Ok(remaining) = query_at.duration_since(now) {
                    sleep_for = sleep_for.min(remaining);
                } else if let Some(duration) = item.uptime.go_down() {
                    states_to_persist.push(item.uptime.clone());
                    tokio::spawn(dispatch_notifications(item.clone(), context.clone(), Some(duration)));
                }
            }
        }
        // Persist state changes to DB
        if !states_to_persist.is_empty() {
            match db_pool.get().await {
                Ok(mut conn) => {
                    for state in &states_to_persist {
                        if let Err(err) = db::update_uptime_state(&mut conn, state).await {
                            warn!("Failed to persist uptime state: {err:?}");
                        }
                    }
                }
                Err(err) => warn!("Failed to get DB connection for state persistence: {err:?}"),
            }
        }
        // @NOTE: This is the default sleep, which handles a case where all clients went
        //  offline, which would be pretty rare at scale, but we must handle this case
        //  anyways, since in case that happens we don't want to run without sleep.
        tokio::time::sleep(sleep_for).await;
    }
}

#[get("/api/v1/health")]
async fn api_health() -> Value {
    // TODO: proper healthcheck.
    return json!({"status": 200});
}

#[get("/api/v1/up")]
async fn api_up(bauth: bauth::BAuth, mut conn: Connection<DB>, context: &State<context::Context>) -> Status {
    let uptime_snapshot = {
        let mut guard = context.users.write().await;
        let Some(item) = guard.get_mut(&bauth.uid) else {
            // User was deleted between BAuth validation and here (race with delete_user)
            return Status::Unauthorized;
        };
        match item.uptime.touch() {
            db::TouchResult::Connected => {
                // Clone with Uninitialized status so dispatch uses "device connected" title
                let mut notification_state = item.clone();
                notification_state.uptime.status = db::UpStatus::Uninitialized;
                tokio::spawn(dispatch_notifications(
                    notification_state,
                    context.inner().clone(),
                    None,
                ));
            }
            db::TouchResult::Restored(duration) => {
                tokio::spawn(dispatch_notifications(
                    item.clone(),
                    context.inner().clone(),
                    Some(duration),
                ));
            }
            db::TouchResult::NoChange => {}
        }
        item.uptime.clone()
    };
    // Persist uptime state to DB (outside the write lock to avoid blocking)
    if let Err(err) = db::update_uptime_state(&mut conn, &uptime_snapshot).await {
        warn!("Failed to persist uptime state: {err:?}");
    }
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
                api::admin_list_users,
                api::admin_get_user,
                api::delete_user,
                api_up,
                api_health,
            ],
        )
        .manage(context::Context::init())
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
            tokio::spawn(background_handle_down(context.clone(), pool));
            Ok(rocket)
        }))
        .ignite()
        .await?
        .launch()
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uk() -> LanguageIdentifier { "uk".parse().unwrap() }
    fn en() -> LanguageIdentifier { "en".parse().unwrap() }

    // Ukrainian duration tests

    #[test]
    fn test_format_duration_uk_minutes_only() {
        let lang = uk();
        assert_eq!(format_duration(&lang, Duration::from_secs(0)), "0 хв");
        assert_eq!(format_duration(&lang, Duration::from_secs(60)), "1 хв");
        assert_eq!(format_duration(&lang, Duration::from_secs(35 * 60)), "35 хв");
        assert_eq!(format_duration(&lang, Duration::from_secs(59 * 60)), "59 хв");
    }

    #[test]
    fn test_format_duration_uk_hours_and_minutes() {
        let lang = uk();
        assert_eq!(format_duration(&lang, Duration::from_secs(3600)), "1 год");
        assert_eq!(format_duration(&lang, Duration::from_secs(3600 + 8 * 60)), "1 год 8 хв");
        assert_eq!(
            format_duration(&lang, Duration::from_secs(23 * 3600 + 8 * 60)),
            "23 год 8 хв"
        );
    }

    #[test]
    fn test_format_duration_uk_days_singular() {
        let lang = uk();
        // 1 день (singular)
        assert_eq!(format_duration(&lang, Duration::from_secs(86400)), "1 день");
        assert_eq!(
            format_duration(&lang, Duration::from_secs(86400 + 12 * 3600 + 22 * 60)),
            "1 день 12 год 22 хв"
        );
        // 21 день, 31 день, etc. (ends in 1, but not 11)
        assert_eq!(format_duration(&lang, Duration::from_secs(21 * 86400)), "21 день");
        assert_eq!(format_duration(&lang, Duration::from_secs(31 * 86400)), "31 день");
        assert_eq!(format_duration(&lang, Duration::from_secs(101 * 86400)), "101 день");
    }

    #[test]
    fn test_format_duration_uk_days_few() {
        let lang = uk();
        // 2-4 дні (few, but not 12-14)
        assert_eq!(format_duration(&lang, Duration::from_secs(2 * 86400)), "2 дні");
        assert_eq!(format_duration(&lang, Duration::from_secs(3 * 86400)), "3 дні");
        assert_eq!(format_duration(&lang, Duration::from_secs(4 * 86400)), "4 дні");
        assert_eq!(format_duration(&lang, Duration::from_secs(22 * 86400)), "22 дні");
        assert_eq!(format_duration(&lang, Duration::from_secs(24 * 86400)), "24 дні");
        assert_eq!(format_duration(&lang, Duration::from_secs(102 * 86400)), "102 дні");
    }

    #[test]
    fn test_format_duration_uk_days_many() {
        let lang = uk();
        // 0, 5-20, 11-14 днів (many)
        assert_eq!(format_duration(&lang, Duration::from_secs(5 * 86400)), "5 днів");
        assert_eq!(format_duration(&lang, Duration::from_secs(10 * 86400)), "10 днів");
        assert_eq!(format_duration(&lang, Duration::from_secs(11 * 86400)), "11 днів");
        assert_eq!(format_duration(&lang, Duration::from_secs(12 * 86400)), "12 днів");
        assert_eq!(format_duration(&lang, Duration::from_secs(13 * 86400)), "13 днів");
        assert_eq!(format_duration(&lang, Duration::from_secs(14 * 86400)), "14 днів");
        assert_eq!(format_duration(&lang, Duration::from_secs(15 * 86400)), "15 днів");
        assert_eq!(format_duration(&lang, Duration::from_secs(20 * 86400)), "20 днів");
        assert_eq!(format_duration(&lang, Duration::from_secs(100 * 86400)), "100 днів");
        assert_eq!(format_duration(&lang, Duration::from_secs(111 * 86400)), "111 днів");
        assert_eq!(format_duration(&lang, Duration::from_secs(112 * 86400)), "112 днів");
    }

    #[test]
    fn test_format_duration_uk_full_example() {
        let lang = uk();
        let duration = Duration::from_secs(238 * 86400 + 15 * 3600 + 13 * 60);
        assert_eq!(format_duration(&lang, duration), "238 днів 15 год 13 хв");
    }

    // English duration tests

    #[test]
    fn test_format_duration_en_minutes_only() {
        let lang = en();
        assert_eq!(format_duration(&lang, Duration::from_secs(0)), "0 min");
        assert_eq!(format_duration(&lang, Duration::from_secs(60)), "1 min");
        assert_eq!(format_duration(&lang, Duration::from_secs(35 * 60)), "35 min");
        assert_eq!(format_duration(&lang, Duration::from_secs(59 * 60)), "59 min");
    }

    #[test]
    fn test_format_duration_en_hours_and_minutes() {
        let lang = en();
        assert_eq!(format_duration(&lang, Duration::from_secs(3600)), "1 hr");
        assert_eq!(format_duration(&lang, Duration::from_secs(3600 + 8 * 60)), "1 hr 8 min");
        assert_eq!(
            format_duration(&lang, Duration::from_secs(23 * 3600 + 8 * 60)),
            "23 hr 8 min"
        );
    }

    #[test]
    fn test_format_duration_en_days() {
        let lang = en();
        // Singular
        assert_eq!(format_duration(&lang, Duration::from_secs(86400)), "1 day");
        assert_eq!(
            format_duration(&lang, Duration::from_secs(86400 + 12 * 3600 + 22 * 60)),
            "1 day 12 hr 22 min"
        );
        // Plural
        assert_eq!(format_duration(&lang, Duration::from_secs(2 * 86400)), "2 days");
        assert_eq!(format_duration(&lang, Duration::from_secs(10 * 86400)), "10 days");
        assert_eq!(format_duration(&lang, Duration::from_secs(21 * 86400)), "21 days");
    }

    #[test]
    fn test_format_duration_en_full_example() {
        let lang = en();
        let duration = Duration::from_secs(238 * 86400 + 15 * 3600 + 13 * 60);
        assert_eq!(format_duration(&lang, duration), "238 days 15 hr 13 min");
    }
}
