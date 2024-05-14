use poise::serenity_prelude::{ChannelId, CreateMessage};

pub async fn server_sync(ctx: &serenity::all::Context) -> Result<(), crate::Error> {
    // Loop over every single guild we currently have
    for guild in ctx.cache.guilds() {}
    Ok(())
}
