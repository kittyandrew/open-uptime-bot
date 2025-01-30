use crate::db::{ChatState, TelegramUser, UptimeState, User, UserState, UserType};
use crate::ntfy;
use crate::{context::Context, db};
use fluent::types::FluentValue;
use fluent_templates::Loader;
use grammers_client::session::Session;
use grammers_client::types::{CallbackQuery, Chat, Message, User as TgUser};
use grammers_client::{Client, Config, InitParams, ReconnectionPolicy};
use grammers_client::{InputMessage, button, reply_markup};
use lazy_static::lazy_static;
use rocket_db_pools::diesel::PgPool;
use std::collections::HashMap;
use std::env::var;
use std::ops::ControlFlow;
use std::time::Duration;
use unic_langid::LanguageIdentifier;

lazy_static! {
    static ref TELEGRAM_SUPERUSER_ID: i64 = var("TELEGRAM_SUPERUSER_ID").unwrap().parse().unwrap();
    static ref SOURCE_CODE_URL: String = var("SOURCE_CODE_URL").unwrap();
    static ref PUBLIC_INVITE_EMAIL: String = var("PUBLIC_INVITE_EMAIL").unwrap();
}

type ResultClient = std::result::Result<Client, Box<dyn std::error::Error>>;
type ResultEmpty = std::result::Result<(), Box<dyn std::error::Error>>;

pub async fn init_client() -> ResultClient {
    struct CustomPolicy;
    impl ReconnectionPolicy for CustomPolicy {
        fn should_retry(&self, attempts: usize) -> ControlFlow<(), Duration> {
            ControlFlow::Continue(Duration::from_millis(u64::pow(attempts as _, 2)))
        }
    }

    let session_fp = var("GRAMMERS_SESSION_FP")?;
    let token = var("GRAMMERS_BOT_TOKEN")?;

    let client = Client::connect(Config {
        session: Session::load_file_or_create(session_fp.clone())?,
        api_id: var("GRAMMERS_API_ID")?.parse()?,
        api_hash: var("GRAMMERS_API_HASH")?,
        params: InitParams {
            reconnection_policy: &CustomPolicy,
            ..Default::default()
        },
    })
    .await?;

    if !client.is_authorized().await? {
        info!("Signing in...");
        client.bot_sign_in(&token).await?;
        client.session().save_to_file(session_fp)?;
        info!("Signed in!");
    }

    info!("Initialized grammers session...");
    return Ok(client);
}

fn get_text_arguments<'a>(message: &Message, state: Option<&UserState>) -> HashMap<&'a str, FluentValue<'a>> {
    // We can do this because we know we are talking to the user.
    let name_mention = match message.chat().username() {
        Some(username) => format!("@{}", username),
        None => format!(
            "<a href=\"tg://user?id={}\">{}</a>",
            message.chat().id(),
            message.chat().name().to_string()
        ),
    };

    let mut map = HashMap::new();
    map.insert("name", name_mention.into());
    map.insert("ntfy_host", ntfy::NTFY_BASE_URL.to_string().into());
    map.insert("invite_email", PUBLIC_INVITE_EMAIL.to_string().into());
    if let Some(state_value) = state {
        map.insert("ntfy_username", state_value.ntfy.username.clone().into());
        map.insert("ntfy_password", state_value.ntfy.password.clone().into());
        map.insert("ntfy_topic", state_value.ntfy.topic.clone().into());
        map.insert("access_token", state_value.user.access_token.clone().into());
    }
    map
}

async fn handle_existing_user(
    state: UserState,
    user: TgUser,
    message: Message,
    context: Context,
    db_pool: PgPool,
) -> ResultEmpty {
    let lang: LanguageIdentifier = state.tg.language_code.parse()?;

    match state.tg.chat_state {
        ChatState::Main => {
            let input_message = {
                let main_args = get_text_arguments(&message, Some(&state));
                let text = crate::LOCALES.lookup_with_args(&lang, "main-message", &main_args);

                let source_text = crate::LOCALES.lookup(&lang, "source-code");

                let ntfy_text = match state.ntfy.enabled {
                    true => format!("ntfy: {}", crate::LOCALES.lookup(&lang, "enabled")),
                    false => format!("ntfy: {}", crate::LOCALES.lookup(&lang, "disabled")),
                };
                let ntfy_payload = format!("v1|ntfy|{}", state.ntfy.enabled);

                let telegram_text = match state.tg.enabled {
                    true => format!("telegram: {}", crate::LOCALES.lookup(&lang, "enabled")),
                    false => format!("telegram: {}", crate::LOCALES.lookup(&lang, "disabled")),
                };
                let telegram_payload = format!("v1|telegram|{}", state.tg.enabled);

                let markup = reply_markup::inline(vec![
                    vec![button::url(source_text, SOURCE_CODE_URL.to_string())],
                    vec![button::inline(ntfy_text, ntfy_payload)],
                    vec![button::inline(telegram_text, telegram_payload)],
                ]);
                InputMessage::html(text).reply_markup(&markup).link_preview(false)
            };
            message.respond(input_message).await?;
        }
        ChatState::Invites => {
            warn!("Unhandled chat state: {state:?}");
        }
    };

    Ok(())
}

async fn handle_new_user(user: TgUser, message: Message, context: Context, db_pool: PgPool) -> ResultEmpty {
    let mut new_user_result = None;
    if user.id() == *TELEGRAM_SUPERUSER_ID {
        let ntfy = context.ntfy.create_new_user(true).await?;
        let tg = TelegramUser::new(false, user.id(), user.lang_code().unwrap().to_string());
        let new_user = User::new(UserType::Admin, 5, None, None, &ntfy, &tg);
        let new_state = UserState {
            user: new_user.clone(),
            ntfy,
            tg,
            uptime: UptimeState::new(new_user.id),
        };
        let mut conn = db_pool.get().await?;
        db::create_new_state(&mut conn, &new_state, None).await?;
        context.add_state(new_state.clone()).await;
        new_user_result = Some(new_state);
        info!("Created admin: {} ({})", user.full_name(), user.id());
    } else if message.text().starts_with("/start") {
        match message.text().split_once(" ") {
            Some((_, value)) if value.len() == 16 => {
                if let Some(invite_id) = context.invite_tokens.write().await.get(value) {
                    info!("Activating invite token: {value} ({invite_id})");
                }
            }
            _ => {}
        };
    }

    let lang: LanguageIdentifier = user.lang_code().unwrap().parse()?;
    match new_user_result {
        Some(state) => {
            let input_message = {
                let main_args = get_text_arguments(&message, Some(&state));
                let text = crate::LOCALES.lookup_with_args(&lang, "new-user-message", &main_args);
                InputMessage::html(text).link_preview(false)
            };
            message.respond(input_message).await?;
        }
        None => {
            let input_message = {
                let main_args = get_text_arguments(&message, None);
                let part_a = crate::LOCALES.lookup_with_args(&lang, "uninvited-message", &main_args);
                let part_b = crate::LOCALES.lookup_with_args(&lang, "invite-part", &main_args);
                InputMessage::html(format!("{}\n\n{}", part_a, part_b)).link_preview(false)
            };
            message.respond(input_message).await?;
        }
    }
    Ok(())
}

pub async fn handle_new_message(message: Message, context: Context, db_pool: PgPool) {
    match message.chat() {
        Chat::User(user_chat) => {
            info!("Responding to user {user_id:?}", user_id = user_chat.id());
            assert!(user_chat.id() > 0, "User cannot have negative ID!");

            // @TODO: Figure out how to lock here per user!?

            if let Some(uid) = context.tg_users.read().await.get(&user_chat.id()) {
                let state = context.users.read().await.get(uid).unwrap().clone();
                if let Err(err) = handle_existing_user(state, user_chat, message, context.clone(), db_pool).await {
                    warn!("Error handling existing user message: {err:?}");
                }
            } else {
                if let Err(err) = handle_new_user(user_chat, message, context.clone(), db_pool).await {
                    warn!("Error handling new user message: {err:?}");
                }
            };
        }
        chat => warn!("Received message in the ignored type: {chat:?}"),
    };
}

pub async fn handle_new_callback(callback: CallbackQuery, context: Context, db_pool: PgPool) {
    let text = "You've clicked: good job!";
    if let Err(err) = callback.answer().alert(text).send().await {
        warn!("Error handling callback query: {err:?}");
    };
}
