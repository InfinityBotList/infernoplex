use serenity::all::{GuildId, UserId};
use sqlx::Row;

use crate::Data;

/// Periodically reconciles infernoplex-managed team memberships against
/// actual Discord guild membership and Administrator status.
///
/// This replaces the old `GuildMemberUpdate`/`GuildMemberRemoval` gateway
/// event handlers. Doing this via REST polling instead of reacting to
/// gateway events means infernoplex never needs the privileged Server
/// Members intent at all: `GET /guilds/{id}` and
/// `GET /guilds/{id}/members/{user}` are both unrestricted REST endpoints,
/// unlike the bulk `List Guild Members` endpoint (which is what the
/// privileged intent actually gates). The tradeoff is reacting on a poll
/// interval instead of instantly.
pub async fn team_cleanup(ctx: &serenity::all::Context) -> Result<(), crate::Error> {
    let data = ctx.data::<Data>();

    let rows = sqlx::query(
        "SELECT tm.team_id, tm.user_id, s.server_id \
         FROM team_members tm \
         JOIN servers s ON s.team_owner = tm.team_id \
         WHERE tm.service = 'infernoplex'",
    )
    .fetch_all(&data.pool)
    .await?;

    for row in rows {
        let team_id: sqlx::types::Uuid = row.try_get("team_id")?;
        let user_id_str: String = row.try_get("user_id")?;
        let server_id_str: String = row.try_get("server_id")?;

        let Ok(guild_id) = server_id_str.parse::<GuildId>() else {
            continue;
        };
        let Ok(user_id) = user_id_str.parse::<UserId>() else {
            continue;
        };

        let member = match ctx.http.get_member(guild_id, user_id).await {
            Ok(member) => member,
            Err(_) => {
                // No longer a member of the guild (or we can't see them
                // anymore for some other reason) - remove them from the team.
                remove_team_member(&data.pool, team_id, user_id).await?;
                continue;
            }
        };

        // Fetched once per guild rather than per member would be more
        // efficient, but this task runs infrequently and team rosters are
        // small, so the simpler per-row shape is fine here.
        let guild = match ctx.http.get_guild(guild_id).await {
            Ok(guild) => guild,
            Err(_) => continue, // Leave this row for the next pass rather than guess.
        };

        let is_admin = member.user.id == guild.owner_id
            || member.roles.iter().any(|role_id| {
                guild
                    .roles
                    .get(role_id)
                    .is_some_and(|role| role.permissions.administrator())
            });

        if !is_admin {
            remove_team_member(&data.pool, team_id, user_id).await?;
        }
    }

    Ok(())
}

async fn remove_team_member(
    pool: &sqlx::PgPool,
    team_id: sqlx::types::Uuid,
    user_id: UserId,
) -> Result<(), crate::Error> {
    sqlx::query(
        "DELETE FROM team_members WHERE team_id = $1 AND user_id = $2 AND service = 'infernoplex'",
    )
    .bind(team_id)
    .bind(user_id.to_string())
    .execute(pool)
    .await?;

    Ok(())
}
