use serenity::all::UserId;

use crate::{Context, Error};

/// Statistics about a guild
pub struct GuildStats {
    pub name: String,
    pub icon: String,
    pub owner: UserId,
    pub total_members: usize,
    pub online_members: usize,
    pub nsfw: bool,
}

impl GuildStats {
    /// Resolved via REST (`GET /guilds/{id}?with_counts=true`) rather than
    /// the gateway cache, so this works without the privileged Server
    /// Members/Presence intents. Member and online counts are Discord's
    /// approximate counts rather than an exact live tally, which is the
    /// tradeoff for not holding a privileged-intent gateway subscription.
    pub async fn from_ctx(ctx: &Context<'_>) -> Result<Self, Error> {
        let guild_id = ctx.guild_id().ok_or("No guild")?;
        let guild = ctx.http().get_guild_with_counts(guild_id).await?;

        Ok(GuildStats {
            name: guild.name.to_string(),
            icon: guild
                .icon_url()
                .unwrap_or_else(|| "https://cdn.discordapp.com/embed/avatars/0.png".to_string()),
            owner: guild.owner_id,
            total_members: guild
                .approximate_member_count
                .map(|n| n.get() as usize)
                .unwrap_or(0),
            online_members: guild
                .approximate_presence_count
                .map(|n| n.get() as usize)
                .unwrap_or(0),
            nsfw: matches!(guild.nsfw_level, serenity::all::NsfwLevel::Explicit),
        })
    }

    pub async fn download_image(&self) -> Result<Vec<u8>, Error> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()?;

        let resp = client.get(&self.icon).send().await?;

        Ok(resp.bytes().await?.to_vec())
    }
}
