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
