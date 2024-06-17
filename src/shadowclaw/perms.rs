pub enum GetMemberTeamPermissionsResult {
    Found(Vec<kittycat::perms::Permission>),
    ServerNotFound,
    MemberNotInTeam,
}

pub async fn get_member_team_permissions(
    pool: &sqlx::PgPool,
    guild_id: serenity::all::GuildId,
    user_id: serenity::all::UserId,
) -> Result<GetMemberTeamPermissionsResult, crate::Error> {
    let res = sqlx::query!(
        "SELECT team_owner FROM servers WHERE server_id = $1",
        guild_id.to_string(),
    )
    .fetch_optional(pool)
    .await?;

    let Some(row) = res else {
        return Ok(GetMemberTeamPermissionsResult::ServerNotFound);
    };

    let team_member_perms = sqlx::query!(
        "SELECT flags FROM team_members WHERE team_id = $1 AND user_id = $2",
        row.team_owner,
        user_id.to_string(),
    )
    .fetch_optional(pool)
    .await?;

    let Some(team_member_perms) = team_member_perms else {
        return Ok(GetMemberTeamPermissionsResult::MemberNotInTeam);
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

    Ok(GetMemberTeamPermissionsResult::Found(sp.resolve()))
}

/// Simple helper method to check for a permission
pub async fn check_for_permission(
    ctx: &crate::Context<'_>,
    perm: &str,
) -> Result<(), crate::Error> {
    let Some(guild_id) = ctx.guild_id() else {
        return Err("This operation can only be performed in a server".into());
    };

    match crate::shadowclaw::perms::get_member_team_permissions(
        &ctx.data().pool,
        guild_id,
        ctx.author().id,
    )
    .await?
    {
        crate::shadowclaw::perms::GetMemberTeamPermissionsResult::Found(permissions) => {
            if !kittycat::perms::has_perm(&permissions, &perm.into()) {
                return Err(format!(
                    "You must have the ``{}`` permission to perform this operation!",
                    perm
                )
                .into());
            }
        }
        crate::shadowclaw::perms::GetMemberTeamPermissionsResult::ServerNotFound => {
            return Err("This server is not on Infinity List! Run `/setup` to enlist it!".into());
        }
        crate::shadowclaw::perms::GetMemberTeamPermissionsResult::MemberNotInTeam => {
            return Err("You are not in this server's team!".into());
        }
    }

    Ok(())
}
