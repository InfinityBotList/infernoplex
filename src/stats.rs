use poise::{serenity_prelude::CreateEmbed, CreateReply};

type Error = crate::Error;
type Context<'a> = crate::Context<'a>;

// Various statistics
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const GIT_SHA: &str = env!("VERGEN_GIT_SHA");
pub const GIT_SEMVER: &str = env!("VERGEN_GIT_SEMVER");
pub const GIT_COMMIT_MSG: &str = env!("VERGEN_GIT_COMMIT_MESSAGE");
pub const BUILD_CPU: &str = env!("VERGEN_SYSINFO_CPU_BRAND");
pub const CARGO_PROFILE: &str = env!("VERGEN_CARGO_PROFILE");
pub const RUSTC_VERSION: &str = env!("VERGEN_RUSTC_SEMVER");

#[poise::command(category = "Stats", prefix_command, slash_command, user_cooldown = 1)]
pub async fn stats(ctx: Context<'_>) -> Result<(), Error> {
    let msg = CreateReply::default().embed(
        CreateEmbed::default()
            .title("Infernoplex Statistics")
            .field("Bot Version:", VERSION, true)
            .field("Rustc Version:", RUSTC_VERSION, true)
            .field(
                "Git Commit:",
                GIT_SHA.to_string() + "(semver=" + GIT_SEMVER + ")",
                true,
            )
            .field("Commit Message:", GIT_COMMIT_MSG, true)
            .field("Cargo Profile:", CARGO_PROFILE, true)
            .field("Built On:", BUILD_CPU, true)
            .field(
                "Current Environment:",
                crate::config::CURRENT_ENV.clone(),
                true,
            ),
    );

    ctx.send(msg).await?;
    Ok(())
}
