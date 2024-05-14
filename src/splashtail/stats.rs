use serenity::all::UserId;

use crate::{Context, Error};

/// Statistics about a guild
pub struct GuildStats {
    pub name: String,
    pub icon: String,
    pub owner: UserId,
    pub total_members: usize,
    pub online_members: usize,
}

impl GuildStats {
    pub fn from_ctx(ctx: &Context) -> Result<Self, Error> {
        let guild = ctx.guild().ok_or("No guild")?;

        Ok(GuildStats {
            name: guild.name.to_string(),
            icon: guild
                .icon_url()
                .unwrap_or_else(|| "https://cdn.discordapp.com/embed/avatars/0.png".to_string()),
            owner: guild.owner_id,
            total_members: guild.members.len(),
            online_members: guild
                .presences
                .iter()
                .filter(|p| p.status != serenity::model::prelude::OnlineStatus::Offline)
                .count(),
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
