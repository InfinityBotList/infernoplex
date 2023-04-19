use poise::{CreateReply, serenity_prelude::{CreateEmbed, CreateActionRow, CreateButton}};

use crate::{crypto, Context, Error};

/// Sets up a server, needs MANAGE_SERVER permissions
#[
    poise::command(
        prefix_command,
        slash_command,
        required_permissions = "MANAGE_GUILD",
    )
]
pub async fn setup(ctx: Context<'_>) -> Result<(), Error> {
    if ctx.guild_id().is_none() {
        ctx.say("This command can only be used in a server.").await?;
        return Ok(());
    }

    let server_id = ctx.guild_id().ok_or("No guild id")?.to_string();

    // Check if the server is already setup
    let res = sqlx::query!(
        "SELECT COUNT(*) FROM servers WHERE server_id = $1",
        server_id
    )
    .fetch_one(&ctx.data().pool)
    .await?;

    if res.count.unwrap_or(0) > 0 {
        ctx.send(
            CreateReply::new()
            .embed(
                CreateEmbed::new()
                .title("Server Already Setup")
                .url(
                    format!("{}/servers/{}", crate::config::CONFIG.frontend_url, server_id)
                )
                .description("You can currently only update servers from the website!")
            )
            .components(
                vec![
                    CreateActionRow::Buttons(
                        vec![
                            CreateButton::new_link(
                                format!("{}/servers/{}", crate::config::CONFIG.frontend_url, server_id),
                            )
                            .label("Redirect")
                        ]
                    )
                ]
            )
        )
        .await?;
        return Ok(());
    }

    ctx.defer().await?;

    // We have to do this to ensure the future stays Send
    let (
        guild_name,
        guild_icon,
        guild_owner_id,
    ) = {
        let guild = ctx.guild().ok_or("No guild")?;

        (
            guild.name.clone(),
            guild.icon_url().unwrap_or_else(|| "https://cdn.discordapp.com/embed/avatars/0.png".to_string()),
            guild.owner_id.to_string(),
        )
    };

    // Create a new team

    let mut tx = ctx.data().pool.begin().await?;

    let team_id = sqlx::query!(
        "INSERT INTO teams (name, avatar) VALUES ($1, $2) RETURNING id",
        format!("{}'s Team", guild_name),
        guild_icon
    )
    .fetch_one(&mut tx)
    .await?;

    let team_id = team_id.id;

    // Check that server owner is a user
    let res = sqlx::query!(
        "SELECT COUNT(*) FROM users WHERE user_id = $1",
        guild_owner_id
    )
    .fetch_one(&mut tx)
    .await?;

    if res.count.unwrap_or(0) == 0 {
        sqlx::query!(
            "INSERT INTO users (user_id, api_token, extra_links, staff, developer, certified) VALUES ($1, $2, $3, false, false, false)",
            guild_owner_id,
            crypto::gen_random(138),
            sqlx::types::JsonValue::Array(vec![]),
        )
        .execute(&mut tx)
        .await?;
    }


    // Add owner with OWNER permission
    sqlx::query!(
        "INSERT INTO team_members (team_id, user_id, perms) VALUES ($1, $2, $3)",
        team_id,
        guild_owner_id,
        &["OWNER".to_string()]
    )
    .execute(&mut tx)
    .await?;

    // Add the user calling the command to the team too but with less perms since theyre a "setup guy"
    
    // First ensure the user is a ibl user
    let res = sqlx::query!(
        "SELECT COUNT(*) FROM users WHERE user_id = $1",
        ctx.author().id.to_string()
    )
    .fetch_one(&mut tx)
    .await?;

    if res.count.unwrap_or(0) == 0 {
        sqlx::query!(
            "INSERT INTO users (user_id, api_token, extra_links, staff, developer, certified) VALUES ($1, $2, $3, false, false, false)",
            ctx.author().id.to_string(),
            crypto::gen_random(138),
            sqlx::types::JsonValue::Array(vec![]),
        )
        .execute(&mut tx)
        .await?;
    }
    
    // Then add to team
    sqlx::query!(
        "INSERT INTO team_members (team_id, user_id, perms) VALUES ($1, $2, $3)",
        team_id,
        ctx.author().id.to_string(),
        &[
            "EDIT_SERVER_SETTINGS".to_string(),
            "SET_SERVER_VANITY".to_string(),
            "CERTIFY_SERVERS".to_string(),
            "RESET_SERVER_TOKEN".to_string(),
            "EDIT_SERVER_WEBHOOKS".to_string(),
            "TEST_SERVER_WEBHOOKS".to_string(),
        ]
    )
    .execute(&mut tx)
    .await?;

    // Create the server
    sqlx::query!(
        "INSERT INTO servers (server_id, name, avatar, team_owner, api_token, vanity) VALUES ($1, $2, $3, $4, $5, $6)",
        server_id,
        guild_name.clone(),
        guild_icon,
        team_id,
        crypto::gen_random(138),
        guild_name + &crypto::gen_random(8)
    )
    .execute(&mut tx)
    .await?;

    tx.commit().await?;

    Ok(())
}