use clap::{Parser, Subcommand};
use serde_json::Value;

#[derive(Parser)]
#[command(name = "oubot-cli")]
#[command(about = "CLI tool for Open Uptime Bot management")]
struct Cli {
    /// Server URL (e.g., http://localhost:8080)
    #[arg(short, long, env = "OUBOT_SERVER")]
    server: String,

    /// Authentication token (not required for 'init' command)
    #[arg(short, long, env = "OUBOT_TOKEN")]
    token: Option<String>,

    /// Output raw JSON instead of formatted view
    #[arg(long, global = true)]
    raw: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize first admin user (no token required) or create a new user with invite
    Init {
        /// Invite token (required for non-first users)
        #[arg(long)]
        invite: Option<String>,
        /// Language code
        #[arg(long, default_value = "uk")]
        language: String,
        /// Number of invites the admin can create (admin only)
        #[arg(long, default_value = "10")]
        invites: i64,
    },

    /// Show current user info
    Me,

    /// Manage client token (for Pico W or similar devices)
    #[command(subcommand)]
    Token(TokenCommands),

    /// Manage ntfy.sh notification settings
    #[command(subcommand)]
    Ntfy(NtfyCommands),

    /// Set notification language (e.g., "uk", "en")
    Language {
        /// Language code (e.g., "uk" for Ukrainian, "en" for English)
        code: String,
    },

    /// Admin commands (requires admin privileges)
    #[command(subcommand)]
    Admin(AdminCommands),
}

#[derive(Subcommand)]
enum TokenCommands {
    /// Show current access token
    Show,
    /// Regenerate access token (WARNING: invalidates current token on all devices)
    Regenerate,
}

#[derive(Subcommand)]
enum NtfyCommands {
    /// Show ntfy.sh notification settings
    Show,
    /// Enable ntfy.sh notifications
    Enable,
    /// Disable ntfy.sh notifications
    Disable,
}

#[derive(Subcommand)]
enum AdminCommands {
    /// List all users
    Users,
    /// Get a specific user
    User {
        /// User ID
        id: String,
    },
    /// List your invites
    Invites,
    /// Create a new invite
    CreateInvite,
    /// Delete an invite
    DeleteInvite {
        /// Invite ID
        id: String,
    },
    /// Delete a user
    DeleteUser {
        /// User ID
        id: String,
    },
}

struct Client {
    http: reqwest::blocking::Client,
    server: String,
    token: Option<String>,
}

impl Client {
    fn new(server: String, token: Option<String>) -> Self {
        Self {
            http: reqwest::blocking::Client::new(),
            server,
            token,
        }
    }

    fn parse_response(resp: reqwest::blocking::Response) -> Result<Value, String> {
        let status = resp.status();

        // Handle common HTTP errors with user-friendly messages
        if status.as_u16() == 401 {
            return Err("Unauthorized: Invalid or missing token".to_string());
        }
        if status.as_u16() == 403 {
            return Err("Forbidden: You don't have permission to access this resource".to_string());
        }
        if status.as_u16() == 404 {
            return Err("Not found: The requested endpoint doesn't exist (is the server up-to-date?)".to_string());
        }
        if status.as_u16() == 429 {
            return Err("Rate limited: Too many requests, please wait".to_string());
        }
        if status.as_u16() >= 500 {
            return Err(format!("Server error (HTTP {})", status));
        }

        let body = resp.text().map_err(|e| format!("Failed to read response: {}", e))?;

        if body.is_empty() {
            return Err(format!("Empty response from server (HTTP {})", status));
        }

        serde_json::from_str(&body).map_err(|e| {
            format!("Failed to parse JSON (HTTP {}): {}", status, e)
        })
    }

    fn get(&self, path: &str) -> Result<Value, String> {
        let url = format!("{}{}", self.server, path);
        let mut req = self.http.get(&url);
        if let Some(token) = &self.token {
            req = req.header("Authorization", format!("token {}", token));
        }
        let resp = req.send().map_err(|e| format!("Request failed: {}", e))?;
        Self::parse_response(resp)
    }

    fn post(&self, path: &str, body: &Value) -> Result<Value, String> {
        let url = format!("{}{}", self.server, path);
        let mut req = self.http.post(&url).json(body);
        if let Some(token) = &self.token {
            req = req.header("Authorization", format!("token {}", token));
        }
        let resp = req.send().map_err(|e| format!("Request failed: {}", e))?;
        Self::parse_response(resp)
    }

    fn post_empty(&self, path: &str) -> Result<Value, String> {
        let url = format!("{}{}", self.server, path);
        let mut req = self.http.post(&url);
        if let Some(token) = &self.token {
            req = req.header("Authorization", format!("token {}", token));
        }
        let resp = req.send().map_err(|e| format!("Request failed: {}", e))?;
        Self::parse_response(resp)
    }

    fn patch(&self, path: &str, body: &Value) -> Result<Value, String> {
        let url = format!("{}{}", self.server, path);
        let mut req = self.http.patch(&url).json(body);
        if let Some(token) = &self.token {
            req = req.header("Authorization", format!("token {}", token));
        }
        let resp = req.send().map_err(|e| format!("Request failed: {}", e))?;
        Self::parse_response(resp)
    }

    fn delete(&self, path: &str) -> Result<Value, String> {
        let url = format!("{}{}", self.server, path);
        let mut req = self.http.delete(&url);
        if let Some(token) = &self.token {
            req = req.header("Authorization", format!("token {}", token));
        }
        let resp = req.send().map_err(|e| format!("Request failed: {}", e))?;
        Self::parse_response(resp)
    }
}

fn print_json(value: &Value) {
    println!("{}", serde_json::to_string_pretty(value).unwrap());
}

// Formatting helpers
mod fmt {
    use serde_json::Value;

    pub fn status_icon(status: &str) -> &'static str {
        match status.to_lowercase().as_str() {
            "up" => "[UP]",
            "down" => "[DOWN]",
            "paused" => "[PAUSED]",
            "uninitialized" => "[INIT]",
            _ => "[?]",
        }
    }

    pub fn bool_icon(v: bool) -> &'static str {
        if v { "[ON]" } else { "[OFF]" }
    }

    pub fn format_timestamp(ts: &str) -> String {
        // Timestamps come as "2026-01-16T03:43:21.123456" - extract date and time
        if let Some(t_idx) = ts.find('T') {
            let date = &ts[..t_idx];
            let time = &ts[t_idx + 1..];
            // Take only HH:MM:SS from time
            let time_short = time.split('.').next().unwrap_or(time);
            format!("{} {}", date, time_short)
        } else {
            ts.to_string()
        }
    }

    pub fn get_str<'a>(v: &'a Value, key: &str) -> &'a str {
        v.get(key).and_then(|x| x.as_str()).unwrap_or("-")
    }

    pub fn get_i64(v: &Value, key: &str) -> i64 {
        v.get(key).and_then(|x| x.as_i64()).unwrap_or(0)
    }

    pub fn get_bool(v: &Value, key: &str) -> bool {
        v.get(key).and_then(|x| x.as_bool()).unwrap_or(false)
    }
}

fn handle_response(result: Result<Value, String>, raw: bool) {
    handle_response_with(result, raw, |json| print_json(json))
}

fn handle_response_with<F>(result: Result<Value, String>, raw: bool, format_fn: F)
where
    F: FnOnce(&Value),
{
    match result {
        Ok(json) => {
            let status = json.get("status").and_then(|s| s.as_i64()).unwrap_or(0);
            if status == 200 {
                if raw {
                    print_json(&json);
                } else {
                    format_fn(&json);
                }
            } else {
                let error = json
                    .get("error")
                    .and_then(|e| e.as_str())
                    .unwrap_or("Unknown error");
                eprintln!("Error: {}", error);
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

// Formatted output functions
fn format_me(json: &Value) {
    if let Some(user_wrapper) = json.get("user") {
        let user = user_wrapper.get("user").unwrap_or(user_wrapper);
        let ntfy = user_wrapper.get("ntfy");

        println!("User Info");
        println!("=========");
        println!("ID:           {}", fmt::get_str(user, "id"));
        println!("Type:         {}", fmt::get_str(user, "user_type"));
        println!("Language:     {}", fmt::get_str(user, "language_code"));
        println!("Created:      {}", fmt::format_timestamp(fmt::get_str(user, "created_at")));
        println!("Up delay:     {}s", fmt::get_i64(user, "up_delay"));
        println!("Invites:      {}/{}", fmt::get_i64(user, "invites_used"), fmt::get_i64(user, "invites_limit"));

        if let Some(ntfy) = ntfy {
            println!();
            println!("Ntfy.sh       {}", fmt::bool_icon(fmt::get_bool(ntfy, "enabled")));
            println!("  Topic:      {}", fmt::get_str(ntfy, "topic"));
            println!("  Username:   {}", fmt::get_str(ntfy, "username"));
        }
    } else {
        print_json(json);
    }
}

fn format_users_list(json: &Value) {
    if let Some(users) = json.get("users").and_then(|u| u.as_array()) {
        if users.is_empty() {
            println!("No users found.");
            return;
        }
        println!("{:<8} {:<36} {:>7} {:>10}",
            "TYPE", "ID", "NTFY", "INVITES");
        println!("{}", "-".repeat(65));
        for user_wrapper in users {
            let user = user_wrapper.get("user").unwrap_or(user_wrapper);
            let ntfy = user_wrapper.get("ntfy");

            let user_type = fmt::get_str(user, "user_type");
            let id = fmt::get_str(user, "id");
            let ntfy_on = ntfy.map(|n| fmt::get_bool(n, "enabled")).unwrap_or(false);
            let invites = format!("{}/{}", fmt::get_i64(user, "invites_used"), fmt::get_i64(user, "invites_limit"));

            println!("{:<8} {:<36} {:>7} {:>10}",
                user_type, id, fmt::bool_icon(ntfy_on), invites);
        }
        println!();
        println!("Total: {} user(s)", users.len());
    } else {
        print_json(json);
    }
}

fn format_invites_list(json: &Value) {
    if let Some(invites) = json.get("invites").and_then(|i| i.as_array()) {
        if invites.is_empty() {
            println!("No invites found.");
            return;
        }
        println!("{:<36} {:<20}",
            "ID", "CREATED");
        println!("{}", "-".repeat(58));
        for invite in invites {
            let id = fmt::get_str(invite, "id");
            let created = fmt::format_timestamp(fmt::get_str(invite, "created_at"));

            println!("{:<36} {:<20}", id, created);
        }
        println!();
        println!("Total: {} invite(s)", invites.len());
    } else {
        print_json(json);
    }
}

fn format_ntfy(json: &Value) {
    if let Some(ntfy) = json.get("ntfy") {
        println!("Ntfy.sh Settings");
        println!("================");
        println!("Enabled:      {}", fmt::bool_icon(fmt::get_bool(ntfy, "enabled")));
        println!("Topic:        {}", fmt::get_str(ntfy, "topic"));
        println!("Username:     {}", fmt::get_str(ntfy, "username"));
        println!("Password:     {}", fmt::get_str(ntfy, "password"));
    } else {
        print_json(json);
    }
}

fn require_token(token: &Option<String>) -> String {
    match token {
        Some(t) => t.clone(),
        None => {
            eprintln!("Error: This command requires authentication. Please provide --token or set OUBOT_TOKEN.");
            std::process::exit(1);
        }
    }
}

fn main() {
    let cli = Cli::parse();
    let client = Client::new(cli.server, cli.token.clone());

    match cli.command {
        Commands::Init { invite, language, invites } => {
            // If invite is provided, create regular user; otherwise create admin (first user)
            let is_admin = invite.is_none();
            let user_type = if is_admin { "Admin" } else { "Normal" };
            let mut body = serde_json::json!({
                "user_type": user_type,
                "invites_limit": invites,
                "ntfy_enabled": true,
                "language_code": language
            });
            if let Some(inv) = invite {
                body["invite"] = serde_json::json!(inv);
            }
            handle_response_with(client.post("/api/v1/users", &body), cli.raw, |json| {
                if let Some(token) = json
                    .get("state")
                    .and_then(|s| s.get("user"))
                    .and_then(|u| u.get("access_token"))
                    .and_then(|t| t.as_str())
                {
                    let label = if is_admin { "Admin" } else { "User" };
                    println!("{} created successfully!", label);
                    println!("Your access token: {}", token);
                    println!("\nUse this token with the CLI:");
                    println!("  export OUBOT_TOKEN={}", token);
                    println!("  oubot-cli me");
                } else {
                    print_json(json);
                }
            });
        }

        Commands::Me => {
            require_token(&cli.token);
            handle_response_with(client.get("/api/v1/me"), cli.raw, format_me);
        }

        Commands::Token(cmd) => {
            require_token(&cli.token);
            match cmd {
                TokenCommands::Show => {
                    match client.get("/api/v1/me") {
                        Ok(json) => {
                            if let Some(token) = json
                                .get("user")
                                .and_then(|u| u.get("user"))
                                .and_then(|u| u.get("access_token"))
                                .and_then(|t| t.as_str())
                            {
                                println!("{}", token);
                            } else {
                                eprintln!("Error: Could not extract token from response");
                                std::process::exit(1);
                            }
                        }
                        Err(e) => {
                            eprintln!("Error: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
                TokenCommands::Regenerate => {
                    handle_response(client.post_empty("/api/v1/me/regenerate-token"), cli.raw);
                }
            }
        }

        Commands::Ntfy(cmd) => {
            require_token(&cli.token);
            match cmd {
                NtfyCommands::Show => {
                    handle_response_with(client.get("/api/v1/me/ntfy"), cli.raw, format_ntfy);
                }
                NtfyCommands::Enable => {
                    let body = serde_json::json!({"enabled": true});
                    handle_response_with(client.patch("/api/v1/me/ntfy", &body), cli.raw, |json| {
                        println!("Ntfy notifications {}", if fmt::get_bool(json, "enabled") { "enabled" } else { "disabled" });
                    });
                }
                NtfyCommands::Disable => {
                    let body = serde_json::json!({"enabled": false});
                    handle_response_with(client.patch("/api/v1/me/ntfy", &body), cli.raw, |json| {
                        println!("Ntfy notifications {}", if fmt::get_bool(json, "enabled") { "enabled" } else { "disabled" });
                    });
                }
            }
        }

        Commands::Language { code } => {
            require_token(&cli.token);
            let body = serde_json::json!({"language_code": code});
            handle_response_with(client.patch("/api/v1/me/language", &body), cli.raw, |json| {
                if let Some(lang) = json.get("language_code").and_then(|l| l.as_str()) {
                    println!("Language set to: {}", lang);
                } else {
                    print_json(json);
                }
            });
        }

        Commands::Admin(cmd) => {
            require_token(&cli.token);
            match cmd {
                AdminCommands::Users => {
                    handle_response_with(client.get("/api/v1/admin/users"), cli.raw, format_users_list);
                }
                AdminCommands::User { id } => {
                    handle_response_with(client.get(&format!("/api/v1/admin/users/{}", id)), cli.raw, format_me);
                }
                AdminCommands::Invites => {
                    handle_response_with(client.get("/api/v1/invites"), cli.raw, format_invites_list);
                }
                AdminCommands::CreateInvite => {
                    handle_response_with(client.post_empty("/api/v1/invites"), cli.raw, |json| {
                        if let Some(token) = json
                            .get("invite")
                            .and_then(|i| i.get("token"))
                            .and_then(|t| t.as_str())
                        {
                            println!("Invite created successfully!");
                            println!("Invite token: {}", token);
                        } else {
                            print_json(json);
                        }
                    });
                }
                AdminCommands::DeleteInvite { id } => {
                    handle_response(client.delete(&format!("/api/v1/invites/{}", id)), cli.raw);
                }
                AdminCommands::DeleteUser { id } => {
                    handle_response(client.delete(&format!("/api/v1/admin/users/{}", id)), cli.raw);
                }
            }
        }
    }
}
