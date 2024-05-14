use crate::Context;
use crate::Error;

#[poise::command(category = "Help", track_edits, prefix_command, slash_command)]
pub async fn help(ctx: Context<'_>, command: Option<String>) -> Result<(), Error> {
    botox::help::help(
        ctx,
        command,
        &crate::config::CONFIG.prefix.get(),
        botox::help::HelpOptions::<crate::Data, u8> {
            state: 0,
            filter: None,
            get_category: None,
        },
    )
    .await
}

#[poise::command(category = "Help", prefix_command, slash_command, user_cooldown = 1)]
pub async fn simplehelp(
    ctx: Context<'_>,
    #[description = "Specific command to show help about"]
    #[autocomplete = "poise::builtins::autocomplete_command"]
    command: Option<String>,
) -> Result<(), Error> {
    botox::help::simplehelp(ctx, command).await
}
