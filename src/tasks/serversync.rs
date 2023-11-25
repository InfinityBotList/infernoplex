use poise::serenity_prelude::{ChannelId, CreateMessage};

pub async fn server_sync(
    pool: &sqlx::PgPool,
    cache_http: &crate::impls::cache::CacheHttpImpl,
) -> Result<(), crate::Error> {
    // Loop over every single guild we currently have 
    for guild in cache_http.cache.guilds() {
        
    }
    Ok(())
}