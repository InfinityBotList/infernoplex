use std::time::Duration;

use poise::{CreateReply, serenity_prelude::{CreateEmbed, CreateActionRow, CreateButton, CreateQuickModal, CreateInputText, InputTextStyle, ButtonStyle}};

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

    let (short, long) = {
        // Create button with confirm+deny
        let builder = CreateReply::default()
        .ephemeral(true)
        .embed(
            CreateEmbed::new()
            .title("Confirm Setup?")
            .description("
This will create a team for your server with the owner as well as the caller of this command as members. You can add more members later.
            
By continuing, you agree that you have read and understood the [Terms of Service](https://infinitybots.gg/legal/terms) and understand that Infinity Development offers no warranty for the use of this product.
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
            .await_component_interaction(ctx.discord())
            .author_id(ctx.author().id)
            .timeout(Duration::from_secs(120))
            .await;

        if let Some(m) = &interaction {
            let id = &m.data.custom_id;

            msg.edit(ctx.discord(), builder.to_prefix_edit().components(vec![]))
                .await?; // remove buttons after button press

            if id == "cancel" {
                return Ok(());
            }

            // Create quick modal asking for short and long for initial setup
            let qm = CreateQuickModal::new("Initial Setup")
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
                    "Long Description",
                    "long",
                )
                .placeholder("Extended server description. If this is above 4096 chars, you can use the website later to update it.")
                .min_length(30)
                .max_length(4096)
            );

            if let Some(resp) = m.quick_modal(ctx.discord(), qm).await? {
                let inputs = resp.inputs;
                let (short, long) = (&inputs[0], &inputs[1]);

                (short.clone(), long.clone())
            } else {
                ctx.send(
                    CreateReply::new()
                    .embed(
                        CreateEmbed::new()
                        .title("Setup Timed Out")
                        .description("Try rerunning this command again to retry setup your server!")
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
                    .description("Try rerunning this command again to retry setup your server!")
                )
                .ephemeral(true)
            ).await?;
    
            return Ok(()); // We dont want to return an error here since it's not an error
        }
    };

    let status_msg = ctx.send(
        CreateReply::new()
        .embed(
            CreateEmbed::new()
            .title("Setting up server...")
            .description("This may take a second, please wait...")
        )
        .ephemeral(true)
    ).await?;

    // We have to do this to ensure the future stays Send
    let (
        guild_name,
        guild_icon,
        guild_owner_id,
        guild_total_members,
        guild_online_members,
    ) = {
        let guild = ctx.guild().ok_or("No guild")?;

        (
            guild.name.clone(),
            guild.icon_url().unwrap_or_else(|| "https://cdn.discordapp.com/embed/avatars/0.png".to_string()),
            guild.owner_id.to_string(),
            guild.members.len(),
            guild.presences.iter().filter(|(_, p)| p.status != serenity::model::prelude::OnlineStatus::Offline).count()
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
        "INSERT INTO servers (
            server_id, 
            name, 
            avatar, 
            team_owner, 
            api_token, 
            vanity,
            total_members,
            online_members,
            short,
            long
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
            $10
        )",
        server_id,
        guild_name.clone(),
        guild_icon,
        team_id,
        crypto::gen_random(138),
        guild_name + &crypto::gen_random(8),
        i32::try_from(guild_total_members)?,
        i32::try_from(guild_online_members)?,
        short,
        long
    )
    .execute(&mut tx)
    .await?;

    tx.commit().await?;

    status_msg.edit(
        ctx,
        CreateReply::new()
        .embed(
            CreateEmbed::new()
            .title("Setting up server...")
            .description("All done :check:")
        )
        .ephemeral(true)
    ).await?;

    Ok(())
}