use std::time::Duration;

use poise::{
    serenity_prelude::{
        ButtonStyle, CreateActionRow, CreateButton, CreateEmbed, CreateInputText,
        CreateInteractionResponse, CreateInteractionResponseMessage, CreateQuickModal,
        InputTextStyle,
    },
    CreateReply,
};

use crate::{Context, Error};

/// Sets up a server, needs 'Administrator' permissions on the author
#[poise::command(prefix_command, slash_command, required_permissions = "ADMINISTRATOR")]
pub async fn setup(ctx: Context<'_>) -> Result<(), Error> {
    let guild: serenity::all::Guild = {
        let Some(cached_guild) = ctx.guild() else {
            return Err("This command must be run in a server!".into());
        };

        cached_guild.clone() // Clone to avoid Sync issues
    };

    let server_id = guild.id.to_string();

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
                .ephemeral(true)
                .embed(
                    CreateEmbed::new()
                        .title("Server Already Setup")
                        .url(format!(
                            "{}/servers/{}",
                            crate::config::CONFIG.frontend_url.get(),
                            server_id
                        ))
                        .description(
                            "Currently, most server settings can only be changed from the website!",
                        ),
                )
                .components(vec![CreateActionRow::Buttons(vec![
                    CreateButton::new_link(format!(
                        "{}/servers/{}",
                        crate::config::CONFIG.frontend_url.get(),
                        server_id
                    ))
                    .label("Redirect"),
                ])]),
        )
        .await?;
        return Ok(());
    }

    let inputs = {
        // Create button with confirm+deny
        let builder = CreateReply::default()
        .embed(
            CreateEmbed::new()
            .title("Confirm Setup?")
            .description("
The following setup will now be performed:

- A new team will be created for your server. The server owner as well as all administrators will then be able to manage this servers listing. You can add more members later through `Team Settings`.
- This server will be added and will be owned by the team. Note that you can transfer ownership of this team to anyone on Infinity List if you want to.
- The server created will be set as a `draft` and will not be visible until it is published.

Notes: 
- If you wish to recover access to this server (rogue moderator/admin etc) within Infinity List, please contact [support](https://infinitybots.gg/redirect/discord)
- **Please now prepare a short and long description for your server.** You can change these later through `Server Settings` on the website.
- By continuing, you agree that you have read and understood the [Terms of Service](https://infinitybots.gg/legal/terms)
            ")
        )
        .components(
            vec![
                CreateActionRow::Buttons(
                    vec![
                        CreateButton::new("next")
                        .label("Next")
                        .style(ButtonStyle::Primary),
                        CreateButton::new("cancel")
                        .label("Cancel")
                        .style(ButtonStyle::Danger)
                    ]
                )
            ]
        );

        let mut msg = ctx.send(builder.clone()).await?.into_message().await?;

        let interaction = msg
            .await_component_interaction(ctx.serenity_context().shard.clone())
            .author_id(ctx.author().id)
            .timeout(Duration::from_secs(360))
            .await;

        if let Some(m) = &interaction {
            let id = &m.data.custom_id;

            msg.edit(
                ctx,
                builder
                    .to_prefix_edit(serenity::all::EditMessage::default())
                    .components(vec![]),
            )
            .await?; // remove buttons after button press

            if id == "cancel" {
                return Ok(());
            }

            // Create quick modal asking for short and long for initial setup
            let qm = CreateQuickModal::new("Initial Setup")
                .field(
                    CreateInputText::new(InputTextStyle::Short, "Vanity", "vanity")
                        .placeholder("This must be unique, so think hard!")
                        .min_length(1)
                        .max_length(20),
                )
                .field(
                    CreateInputText::new(InputTextStyle::Short, "Short Description", "bot_id")
                        .placeholder("Something short and snazzy to brag about!")
                        .min_length(20)
                        .max_length(100),
                )
                .field(
                    CreateInputText::new(
                        InputTextStyle::Paragraph,
                        "Long/Extended Description",
                        "long",
                    )
                    .placeholder("Both markdown and HTML are supported!")
                    .min_length(30)
                    .max_length(4000),
                );

            if let Some(resp) = m.quick_modal(ctx.serenity_context(), qm).await? {
                let inputs = resp.inputs;

                resp.interaction
                    .create_response(
                        ctx.http(),
                        CreateInteractionResponse::Message(
                            CreateInteractionResponseMessage::default().embed(
                                CreateEmbed::new()
                                    .title("Setting up server...")
                                    .description("This may take a second, please wait..."),
                            ),
                        ),
                    )
                    .await?;

                inputs
            } else {
                ctx.send(
                    CreateReply::new()
                        .embed(
                            CreateEmbed::new()
                                .title("Modal Timed Out")
                                .description("Please rerun `/setup`!"),
                        )
                        .ephemeral(true),
                )
                .await?;

                return Ok(()); // We dont want to return an error here since it's not an error
            }
        } else {
            ctx.send(
                CreateReply::new()
                    .embed(
                        CreateEmbed::new()
                            .title("Setup Timed Out")
                            .description("Please rerun `/setup`!"),
                    )
                    .ephemeral(true),
            )
            .await?;

            return Ok(()); // We dont want to return an error here since it's not an error
        }
    };

    // Next try to resolve an invite for this guild
    let invite = crate::shadowclaw::invite::setup_invite_view(&ctx).await?;

    // Get guild stats
    let guild_stats = crate::shadowclaw::stats::GuildStats::from_ctx(&ctx)?;

    // Create a new team with a random vanity
    let mut tx = ctx.data().pool.begin().await?;

    let team_id = sqlx::types::uuid::Uuid::new_v4();
    let team_vanity = botox::crypto::gen_random(256);

    let vanity_tag = sqlx::query!(
        "INSERT INTO vanity (code, target_id, target_type) VALUES ($1, $2, $3) RETURNING itag",
        team_vanity,
        team_id.to_string(),
        "team"
    )
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query!(
        "INSERT INTO teams (id, name, vanity_ref, service) VALUES ($1, $2, $3, 'infernoplex')",
        team_id,
        format!("{}'s Team", guild_stats.name),
        vanity_tag.itag
    )
    .execute(&mut *tx)
    .await?;

    // Save team avatar to {cdn_main_scope_path}/avatars/teams/{team_id}.webp
    let img_bytes = guild_stats.download_image().await?;

    // Convert img_bytes to webp for both teams and servers
    crate::shadowclaw::webp::image_to_webp(
        &guild_stats.icon,
        format!(
            "{}/avatars/teams/{}.webp",
            crate::config::CONFIG.cdn_main_scope_path,
            team_id
        ),
        &img_bytes.clone(),
    )
    .map_err(|e| format!("Error converting image to webp [teams]: {}", e))?;

    // Convert img_bytes to webp for both teams and servers
    crate::shadowclaw::webp::image_to_webp(
        &guild_stats.icon,
        format!(
            "{}/avatars/servers/{}.webp",
            crate::config::CONFIG.cdn_main_scope_path,
            server_id
        ),
        &img_bytes.clone(),
    )
    .map_err(|e| format!("Error converting image to webp [servers]: {}", e))?;

    // Check that server owner is a user
    let res = sqlx::query!(
        "SELECT COUNT(*) FROM users WHERE user_id = $1",
        guild_stats.owner.to_string()
    )
    .fetch_one(&mut *tx)
    .await?;

    if res.count.unwrap_or(0) == 0 {
        sqlx::query!(
            "INSERT INTO users (user_id, extra_links, developer, certified) VALUES ($1, $2, false, false)",
            guild_stats.owner.to_string(),
            sqlx::types::JsonValue::Array(vec![]),
        )
        .execute(&mut *tx)
        .await?;
    }

    // Add owner with Global Owner permission
    sqlx::query!(
        "INSERT INTO team_members (team_id, user_id, flags, service) VALUES ($1, $2, $3, 'infernoplex')",
        team_id,
        guild_stats.owner.to_string(),
        &["global.*".to_string()]
    )
    .execute(&mut *tx)
    .await?;

    // Add all administrators
    for member in guild.members {
        if member.user.id == guild_stats.owner || member.user.bot() {
            continue;
        }

        let member_permissions = member.permissions(ctx.cache())?;

        if member_permissions.administrator() {
            // Then add administrator to team
            // First ensure the user is a ibl user
            let res = sqlx::query!(
                "SELECT COUNT(*) FROM users WHERE user_id = $1",
                member.user.id.to_string()
            )
            .fetch_one(&mut *tx)
            .await?;

            if res.count.unwrap_or(0) == 0 {
                sqlx::query!(
                    "INSERT INTO users (user_id, extra_links, developer, certified) VALUES ($1, $2, false, false)",
                    member.user.id.to_string(),
                    sqlx::types::JsonValue::Array(vec![]),
                )
                .execute(&mut *tx)
                .await?;
            }

            sqlx::query!(
                "INSERT INTO team_members (team_id, user_id, flags, service) VALUES ($1, $2, $3, 'infernoplex')",
                team_id,
                member.user.id.to_string(),
                &["server.*".to_string(),]
            )
            .execute(&mut *tx)
            .await?;
        }
    }

    // Create a vanity for the server
    let vanity_count = sqlx::query!(
        "SELECT COUNT(*) FROM vanity WHERE code::text = $1",
        &inputs[0].to_string()
    )
    .fetch_one(&mut *tx)
    .await?;

    if vanity_count.count.unwrap_or(0) > 0 {
        ctx.send(
            CreateReply::new()
                .embed(
                    CreateEmbed::new()
                        .title("Vanity Already Exists")
                        .description("Please rerun `/setup` with a new vanity!"),
                )
                .ephemeral(true),
        )
        .await?;
        return Ok(());
    }

    let vanity_tag = sqlx::query!(
        "INSERT INTO vanity (code, target_id, target_type) VALUES ($1::text, $2, $3) RETURNING itag",
        &inputs[0].to_string(),
        server_id,
        "server"
    )
    .fetch_one(&mut *tx)
    .await?;

    // Create the server
    sqlx::query!(
        "INSERT INTO servers (
            server_id, 
            name, 
            team_owner, 
            total_members,
            online_members,
            short,
            long,
            invite,
            vanity_ref,
            extra_links,
            nsfw
        ) VALUES (
            $1, 
            $2, 
            $3, 
            $4, 
            $5, 
            $6,
            $7,
            $8,
            $9,
            $10,
            $11
        )",
        server_id,
        guild_stats.name.to_string(),
        team_id,
        i32::try_from(guild_stats.total_members)?,
        i32::try_from(guild_stats.online_members)?,
        &inputs[1].to_string(),
        &inputs[2].to_string(),
        invite,
        vanity_tag.itag,
        serde_json::Value::Array(vec![]),
        guild_stats.nsfw
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    ctx.send(
        CreateReply::new().embed(
            CreateEmbed::new()
                .title("All Done!")
                .description("All done :white_check_mark: "),
        ),
    )
    .await?;

    Ok(())
}
