use clap::Subcommand;

#[derive(Subcommand)]
pub enum Commands {
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

    /// Pause monitoring (freeze state, suppress notifications)
    Pause,

    /// Resume monitoring (restore pre-pause state)
    Unpause,

    /// Manage monitoring settings (up_delay, maintenance window)
    #[command(subcommand)]
    Settings(SettingsCommands),

    /// Admin commands (requires admin privileges)
    #[command(subcommand)]
    Admin(AdminCommands),
}

#[derive(Subcommand)]
pub enum SettingsCommands {
    /// Show current settings
    Show,
    /// Set heartbeat timeout (seconds before device is considered down)
    Delay {
        /// Timeout in seconds (minimum 10)
        seconds: i16,
    },
    /// Set or clear daily maintenance window (times in HH:MM UTC)
    Maintenance {
        /// Start time in HH:MM UTC (omit both times to clear)
        start: Option<String>,
        /// End time in HH:MM UTC
        end: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum TokenCommands {
    /// Show current access token
    Show,
    /// Regenerate access token (WARNING: invalidates current token on all devices)
    Regenerate,
}

#[derive(Subcommand)]
pub enum NtfyCommands {
    /// Show ntfy.sh notification settings
    Show,
    /// Enable ntfy.sh notifications
    Enable,
    /// Disable ntfy.sh notifications
    Disable,
}

#[derive(Subcommand)]
pub enum AdminCommands {
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
