use serenity::all::GuildId;
use sqlx::types::Json;

use crate::Data;

#[derive(serde::Serialize)]
struct SyncedEmoji {
    id: String,
    name: String,
    animated: bool,
    url: String,
}

#[derive(serde::Serialize)]
struct SyncedSticker {
    id: String,
    name: String,
    format: String,
    url: String,
}

pub async fn server_sync(ctx: &serenity::all::Context) -> Result<(), crate::Error> {
    let data = ctx.data::<Data>();

    // Every listed server gets its icon kept fresh (unlike emoji/sticker
    // sync below, this isn't gated behind show_emojis — an icon isn't the
    // kind of thing an owner needs to opt into showing).
    sync_avatars(ctx).await?;

    let rows = sqlx::query("SELECT server_id FROM servers WHERE show_emojis = true")
        .fetch_all(&data.pool)
        .await?;

    for row in rows {
        let server_id: String = sqlx::Row::try_get(&row, "server_id")?;

        let Ok(guild_id) = server_id.parse::<GuildId>() else {
            continue;
        };

        let Ok(emojis) = ctx.http.get_emojis(guild_id).await else {
            continue;
        };

        let Ok(stickers) = ctx.http.get_guild_stickers(guild_id).await else {
            continue;
        };

        let synced_emojis: Vec<SyncedEmoji> = emojis
            .iter()
            .map(|e| SyncedEmoji {
                id: e.id.to_string(),
                name: e.name.to_string(),
                animated: e.animated(),
                url: e.url(),
            })
            .collect();

        let synced_stickers: Vec<SyncedSticker> = stickers
            .iter()
            .filter_map(|s| {
                let url = s.image_url()?;
                Some(SyncedSticker {
                    id: s.id.to_string(),
                    name: s.name.to_string(),
                    format: format!("{:?}", s.format_type).to_lowercase(),
                    url,
                })
            })
            .collect();

        sqlx::query(
            "UPDATE servers SET emojis = $2, stickers = $3, emojis_synced_at = NOW() WHERE server_id = $1",
        )
        .bind(&server_id)
        .bind(Json(&synced_emojis))
        .bind(Json(&synced_stickers))
        .execute(&data.pool)
        .await?;
    }

    Ok(())
}

/// Refreshes every listed server's icon from the gateway cache. No REST call
/// is made — the cache reflects live gateway state, so a server the bot
/// isn't currently a member of is simply skipped rather than erroring.
async fn sync_avatars(ctx: &serenity::all::Context) -> Result<(), crate::Error> {
    let data = ctx.data::<Data>();

    let rows = sqlx::query("SELECT server_id FROM servers")
        .fetch_all(&data.pool)
        .await?;

    for row in rows {
        let server_id: String = sqlx::Row::try_get(&row, "server_id")?;

        let Ok(guild_id) = server_id.parse::<GuildId>() else {
            continue;
        };

        // Scoped so the cache guard is dropped before the .await below.
        let avatar = {
            let Some(guild) = ctx.cache.guild(guild_id) else {
                continue;
            };
            guild.icon_url().unwrap_or_default()
        };

        sqlx::query("UPDATE servers SET avatar = $2 WHERE server_id = $1")
            .bind(&server_id)
            .bind(&avatar)
            .execute(&data.pool)
            .await?;
    }

    Ok(())
}
