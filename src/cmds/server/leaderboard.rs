use crate::{Context, Error};
use serenity::all::Mentionable;

/// Get the users who have voted the most for your server
#[poise::command(prefix_command, slash_command, guild_cooldown = 3)]
pub async fn leaderboard(
    ctx: Context<'_>,
    #[description = "How many results to render."] limit: Option<i64>,
    #[description = "Filter to only those in the server?"] filter_onlycurrmembers: Option<bool>,
) -> Result<(), Error> {
    let Some(guild_id) = ctx.guild_id() else {
        return Err("This command can only be used in a server.".into());
    };

    let data = ctx.data();
    let limit = limit.unwrap_or(10);

    let filter_onlycurrmembers = filter_onlycurrmembers.unwrap_or(false);

    if filter_onlycurrmembers
        && !data
            .intents
            .contains(serenity::all::GatewayIntents::GUILD_MEMBERS)
    {
        return Err("This command requires the GUILD_MEMBERS intent.".into());
    }

    let rec = sqlx::query!(
        r#"
        SELECT author,
        (SUM(CASE WHEN upvote THEN 1 ELSE 0 END) - SUM(CASE WHEN NOT upvote THEN 1 ELSE 0 END)) AS score
        FROM entity_votes 
        WHERE target_id = $1
        AND target_type = 'server'
        AND void = false
        GROUP BY author
        ORDER BY score DESC
        "#,
        guild_id.to_string(),
    )
    .fetch_all(&data.pool)
    .await?;

    let mut leaderboard = Vec::new();

    {
        let Some(guild) = ctx.guild() else {
            return Err("This command can only be used in a server.".into());
        };

        for row in rec.iter() {
            if filter_onlycurrmembers {
                let user_id = row.author.parse::<serenity::all::UserId>()?;

                // Fetch user from guild.members
                if guild.members.contains_key(&user_id) {
                    leaderboard.push(row);
                }

                if leaderboard.len() >= limit as usize {
                    break;
                }
            } else {
                leaderboard.push(row);

                if leaderboard.len() >= limit as usize {
                    break;
                }
            }
        }
    }

    let mut response = String::new();

    let mut page = 1;
    for (i, row) in leaderboard.iter().enumerate() {
        let user_id = row.author.parse::<serenity::all::UserId>()?;

        let next_str = format!(
            "{}. {} - {}\n",
            i + 1,
            user_id.mention(),
            row.score.unwrap_or(0)
        );

        if response.len() + next_str.len() > 4000 {
            ctx.send(
                poise::CreateReply::new().embed(
                    serenity::all::CreateEmbed::new()
                        .title(format!("Leaderboard (Page {})", page))
                        .description(response.clone()),
                ),
            )
            .await?;
            page += 1;
            response.clear();
        }

        response.push_str(&next_str);
    }

    if !response.is_empty() {
        ctx.send(
            poise::CreateReply::new().embed(
                serenity::all::CreateEmbed::new()
                    .title(format!("Leaderboard (Page {})", page + 1))
                    .description(response),
            ),
        )
        .await?;
    }

    Ok(())
}
