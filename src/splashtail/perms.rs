pub async fn get_member_team_permissions(
    pool: &sqlx::PgPool,
    guild_id: serenity::all::GuildId,
    user_id: serenity::all::UserId,
) -> Result<Vec<kittycat::perms::Permission>, crate::Error> {
    let res = sqlx::query!(
        "SELECT team_owner FROM servers WHERE server_id = $1",
        guild_id.to_string(),
    )
    .fetch_optional(pool)
    .await?;

    let Some(row) = res else {
        return Ok(vec![]);
    };

    let team_member_perms = sqlx::query!(
        "SELECT flags FROM team_members WHERE team_id = $1 AND user_id = $2",
        row.team_owner,
        user_id.to_string(),
    )
    .fetch_optional(pool)
    .await?;

    let Some(team_member_perms) = team_member_perms else {
        return Ok(vec![]);
    };

    // Right now, team permissions are treated as permission overrides
    // TODO: support hierarchy based permissions in the future
    let sp = kittycat::perms::StaffPermissions {
        user_positions: vec![],
        perm_overrides: team_member_perms
            .flags
            .into_iter()
            .map(|f| f.into())
            .collect(),
    };

    Ok(sp.resolve())
}
