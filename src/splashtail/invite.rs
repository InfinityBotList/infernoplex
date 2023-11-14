use std::time::Duration;

use crate::{Error, Context};
use log::info;
use poise::CreateReply;
use serenity::{all::{CreateEmbed, CreateActionRow, CreateButton, ButtonStyle, InputTextStyle}, builder::{CreateInputText, CreateInteractionResponse, CreateInteractionResponseMessage, EditInteractionResponse}, utils::CreateQuickModal, prelude::CacheHttp};
use sqlx::types::chrono;

/// Sets up the invite for a server
/// 
/// This returns a string which is the invite selected by the user
pub async fn setup_invite_view(ctx: &Context<'_>) -> Result<String, Error> {
    let builder = CreateReply::new()
        .embed(
            CreateEmbed::new()
            .title("Invite Setup")
            .description("
OK, lets setup the invite for this server! To get started choose which type of invite you would like

- **Invite URL** - Use a (permanent) invite link of your choice
- **Per-User Invite** - Infinity Bot List will create an invite for this server for each user
- **None** - This server will not be invitable. Useful if you wish to use a whitelist form and manually send out invites
    "
            )
        )
        .components(
            vec![
                CreateActionRow::Buttons(
                    vec![
                        CreateButton::new("invite_url")
                        .label("Invite URL")
                        .style(ButtonStyle::Primary),
                        CreateButton::new("per_user")
                        .label("Per-User Invite")
                        .style(ButtonStyle::Primary),
                        CreateButton::new("none")
                        .label("No Invites")
                        .style(ButtonStyle::Primary),
                    ]
                ),
                CreateActionRow::Buttons(
                    vec![
                        CreateButton::new("p1")
                        .label("-")
                        .disabled(true),
                        CreateButton::new("cancel")
                        .label("Cancel")
                        .style(ButtonStyle::Danger),
                        CreateButton::new("p2")
                        .label("-")
                        .disabled(true),
                    ]
                )
            ]
        );
    
    let mut msg: serenity::all::Message = ctx.send(builder.clone()).await?.into_message().await?;

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
            return Err("Setup cancelled".into());
        }

        if id == "invite_url" {
            // Ask for invite url now
            let qm = CreateQuickModal::new("Invite URL Selection")
                .field(
                    CreateInputText::new(
                        InputTextStyle::Short,
                        "Enter Invite URL",
                        "invite_url",
                    )
                    .placeholder("Please enter the invite URL you wish to use!")
                    .min_length(20)
                    .max_length(100)
                );
            
            if let Some(resp) = m.quick_modal(ctx.serenity_context(), qm).await? {
                let inputs = resp.inputs;

                let invite_url = &inputs[0];

                resp.interaction.create_response(
                    &ctx,
                    CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::default()
                        .embed(
                            CreateEmbed::new()
                            .title("Resolving invite")
                            .description(
                                format!("Please wait while we try to resolve this invite: {}", invite_url)
                            )
                        )
                    )
                ).await?;                    

                if let Err(e) = resolve_invite(ctx, invite_url).await {
                    resp.interaction.edit_response(
                        &ctx,
                        EditInteractionResponse::new()
                            .embed(
                                CreateEmbed::new()
                                .title("Error resolving invite")
                                .description(
                                    format!("This invite could not be resolved: {}", e)
                                )
                            )
                    ).await?;

                    return Err(
                        format!(
                            "Error resolving invite: {}",
                            e
                        ).into()
                    );                        
                }

                resp.interaction.edit_response(
                    &ctx,
                    EditInteractionResponse::new()
                        .embed(
                            CreateEmbed::new()
                            .title("Resolved invite successfully!")
                            .description(
                                format!("You have inputted: {}", invite_url)
                            )
                        )
                ).await?;

                return Ok(id.to_owned() + ":" + invite_url);
            } else {
                return Err("Timed out waiting for response for invite URL".into());
            }
        }

        m.create_response(
            &ctx,
            CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::default()
                .embed(
                    CreateEmbed::new()
                    .title("Invite Use")
                    .description(
                        format!("**Chosen invite type:** {}", id)
                    )
                )
            )
        ).await?;            

        Ok(id.clone())
    } else {
        Err("Timed out waiting for choice".into())
    }
}

async fn resolve_invite(ctx: &Context<'_>,  invite: &str) -> Result<(), Error> {
    // Follow all redirects until reaching end
    let client = reqwest::Client::builder()
    .timeout(Duration::from_secs(10))
    .build()?;
    let resp = client.get(invite).send().await?;

    info!("Response: {:?}", resp);

    let url = resp.url().to_string();

    if !url.starts_with("https://discord.com/invite/") && !url.starts_with("https://discord.gg") {
        return Err("Invalid invite URL".into());
    }

    let code = url.split('/').last().ok_or("Invalid invite URL: No code could be parsed")?;
    
    let invite = ctx.http().get_invite(code, false, true, None).await.map_err(
        |e| format!("Failed to fetch invite: {}", e)
    )?;

    if let Some(e) = invite.expires_at {
        // Check length to ensure that expiry is at least 30 days
        let length = e.signed_duration_since(chrono::Utc::now()).num_days();

        if length < 30 {
            return Err("Invite expiry must be after at least 30 days long".into());
        }

        return Err("Invite must be permanent".into());
    }

    Ok(())
}