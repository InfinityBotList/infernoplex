use poise::{CreateReply, serenity_prelude::{CreateEmbed, CreateActionRow, CreateButton, ButtonStyle}};

use crate::{Context, Error};

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

    Ok(())
}