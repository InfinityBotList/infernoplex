use crate::Context;
use crate::Error;

#[poise::command(category = "Help", track_edits, slash_command)]
pub async fn help(ctx: Context<'_>, command: Option<String>) -> Result<(), Error> {
    botox::help::help(
        ctx,
        command,
        &crate::config::CONFIG.prefix.get(),
        botox::help::HelpOptions {
            get_category: None,
        },
    )
    .await
}
