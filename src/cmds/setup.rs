use std::time::Duration;

use poise::{CreateReply, serenity_prelude::{CreateEmbed, CreateActionRow, CreateButton, CreateQuickModal, CreateInputText, InputTextStyle, ButtonStyle, CreateInteractionResponse, CreateInteractionResponseMessage}};

use crate::{crypto, Context, Error};

/// Sets up a server, needs 'Manage Server' permissions
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
            .ephemeral(true)
            .embed(
                CreateEmbed::new()
                .title("Server Already Setup")
                .url(
                    format!("{}/servers/{}", crate::config::CONFIG.frontend_url, server_id)
                )
                .description("Currently, most server settings can only be changed from the website!")
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

    let inputs = {
        // Create button with confirm+deny
        let builder = CreateReply::default()
        .embed(
            CreateEmbed::new()
            .title("Confirm Setup?")
            .description("
The following setup will now be performed:

- A new team will be created for your server. The server owner and the caller of this command will be its initial members. You can add more members later through `Team Settings`.
- This server will be added and will be owned by the team. Note that you can transfer ownership of this team to anyone on Infinity Bot List if you want to.
- The server created will be set as a `draft` and will not be visible until it is published.

Notes: 
- If you wish to recover access to this server (rogue moderator/admin etc) within Infinity Bot List, please contact [support](https://infinitybots.gg/redirect/discord)
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
            .await_component_interaction(ctx)
            .author_id(ctx.author().id)
            .timeout(Duration::from_secs(360))
            .await;

        if let Some(m) = &interaction {
            let id = &m.data.custom_id;

            msg.edit(ctx, builder.to_prefix_edit().components(vec![]))
                .await?; // remove buttons after button press

            if id == "cancel" {
                return Ok(());
            }

            // Create quick modal asking for short and long for initial setup
            let qm = CreateQuickModal::new("Initial Setup")
            .field(
                CreateInputText::new(
                    InputTextStyle::Short,
                    "Vanity",
                    "vanity",
                )
                .placeholder("This must be unique, so think hard!")
                .min_length(1)
                .max_length(20)
            )
            .field(
                CreateInputText::new(
                    InputTextStyle::Short,
                    "Short Description",
                    "bot_id",
                )
                .placeholder("Something short and snazzy to brag about!")
                .min_length(20)
                .max_length(100)
            )
            .field(
                CreateInputText::new(
                    InputTextStyle::Paragraph,
                    "Long/Extended Description",
                    "long",
                )
                .placeholder("Both markdown and HTML are supported!")
                .min_length(30)
                .max_length(4000)
            );

            if let Some(resp) = m.quick_modal(ctx.serenity_context(), qm).await? {
                let inputs = resp.inputs;

                resp.interaction.create_response(
                    &ctx,
                    CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::default()
                        .embed(
                            CreateEmbed::new()
                            .title("Setting up server...")
                            .description("This may take a second, please wait...")
                        )
                    )
                ).await?;            

                inputs
            } else {
                ctx.send(
                    CreateReply::new()
                    .embed(
                        CreateEmbed::new()
                        .title("Modal Timed Out")
                        .description("Please rerun `/setup`!")
                    )
                    .ephemeral(true)
                ).await?;
        
                return Ok(()); // We dont want to return an error here since it's not an error
            }
        } else {
            ctx.send(
                CreateReply::new()
                .embed(
                    CreateEmbed::new()
                    .title("Setup Timed Out")
                    .description("Please rerun `/setup`!")
                )
                .ephemeral(true)
            ).await?;
    
            return Ok(()); // We dont want to return an error here since it's not an error
        }
    };

    // Next try to resolve an invite for this guild
    let invite = crate::splashtail::invite::setup_invite_view(&ctx).await?;

    // Get guild stats
    let guild_stats = crate::splashtail::stats::GuildStats::from_ctx(&ctx)?;

    // Create a new team
    let mut tx = ctx.data().pool.begin().await?;

    let team_id = sqlx::query!(
        "INSERT INTO teams (name) VALUES ($1) RETURNING id",
        format!("{}'s Team", guild_stats.name),
    )
    .fetch_one(&mut tx)
    .await?;

    // Save team avatar to {cdn_main_scope_path}/avatars/teams/{team_id}.webp
    let img_bytes = guild_stats.download_image().await?;

    // Convert img_bytes to webp
    let img_webp_bytes = crate::splashtail::webp::image_to_webp(&guild_stats.icon, &img_bytes).map_err(|e| format!("Error converting image to webp: {}", e))?;

    // Save to cdn
    std::fs::write(
        format!(
            "{}/avatars/teams/{}.webp",
            crate::config::CONFIG.cdn_main_scope_path,
            team_id.id
        ),
        img_webp_bytes
    ).map_err(|e| format!("Error saving team avatar to cdn: {}", e))?;

    // Check that server owner is a user
    let res = sqlx::query!(
        "SELECT COUNT(*) FROM users WHERE user_id = $1",
        guild_stats.owner.to_string()
    )
    .fetch_one(&mut tx)
    .await?;

    if res.count.unwrap_or(0) == 0 {
        sqlx::query!(
            "INSERT INTO users (user_id, api_token, extra_links, staff, developer, certified) VALUES ($1, $2, $3, false, false, false)",
            guild_stats.owner.to_string(),
            crypto::gen_random(138),
            sqlx::types::JsonValue::Array(vec![]),
        )
        .execute(&mut tx)
        .await?;
    }

    // Add owner with Global Owner permission
    sqlx::query!(
        "INSERT INTO team_members (team_id, user_id, flags) VALUES ($1, $2, $3)",
        team_id.id,
        guild_stats.owner.to_string(),
        &["global.*".to_string()]
    )
    .execute(&mut tx)
    .await?;

    // Add the user calling the command to the team
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
        "INSERT INTO team_members (team_id, user_id, flags) VALUES ($1, $2, $3)",
        team_id.id,
        ctx.author().id.to_string(),
        &[
            "global.*".to_string(),
        ]
    )
    .execute(&mut tx)
    .await?;

    // Create a vanity for the server
    let vanity_count = sqlx::query!(
        "SELECT COUNT(*) FROM vanity WHERE code::text = $1",
        inputs[0]
    )
    .fetch_one(&mut tx)
    .await?;

    if vanity_count.count.unwrap_or(0) > 0 {
        ctx.send(
            CreateReply::new()
            .embed(
                CreateEmbed::new()
                .title("Vanity Already Exists")
                .description("Please rerun `/setup` with a new vanity!")
            )
            .ephemeral(true)
        ).await?;
        return Ok(());
    }

    let vanity_tag = sqlx::query!(
        "INSERT INTO vanity (code, target_id, target_type) VALUES ($1::text, $2, $3) RETURNING itag",
        inputs[0],
        server_id,
        "server"
    )
    .fetch_one(&mut tx)
    .await?;

    // Create the server
    sqlx::query!(
        "INSERT INTO servers (
            server_id, 
            name, 
            avatar, 
            team_owner, 
            api_token, 
            total_members,
            online_members,
            short,
            long,
            invite,
            vanity_ref,
            extra_links
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
            $11,
            $12
        )",
        server_id,
        guild_stats.name.clone(),
        guild_stats.icon,
        team_id.id,
        crypto::gen_random(138),
        i32::try_from(guild_stats.total_members)?,
        i32::try_from(guild_stats.online_members)?,
        &inputs[1],
        &inputs[2],
        invite,
        vanity_tag.itag,
        sqlx::types::JsonValue::Array(vec![])
    )
    .execute(&mut tx)
    .await?;

    tx.commit().await?;

    ctx.send(
        CreateReply::new()
        .embed(
            CreateEmbed::new()
            .title("All Done!")
            .description("All done :white_check_mark: ")
        )
    ).await?;

    Ok(())
}