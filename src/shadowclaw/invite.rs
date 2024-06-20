use std::time::Duration;

use crate::{Context, Error};
use log::info;
use poise::CreateReply;
use serde::{Deserialize, Serialize};
use serenity::{
    all::{ButtonStyle, CreateActionRow, CreateButton, CreateEmbed, InputTextStyle},
    builder::{
        CreateInputText, CreateInteractionResponse, CreateInteractionResponseMessage,
        EditInteractionResponse,
    },
    prelude::CacheHttp,
    utils::CreateQuickModal,
};
use sqlx::types::chrono;
use strum_macros::VariantNames;
use ts_rs::TS;
use utoipa::ToSchema;

/// Sets up the invite for a server
///
/// This returns a string which is the invite selected by the user
pub async fn setup_invite_view(ctx: &Context<'_>) -> Result<String, Error> {
    let Some(guild_id) = ctx.guild_id() else {
        return Err("This operation can only be performed in a server".into());
    };

    let builder = CreateReply::new()
        .embed(
            CreateEmbed::new()
            .title("Invite Setup")
            .description("
Okay! Now, let's setup the invite for this server! To get started, choose which type of invite you would like

- **Invite URL** - Use a (permanent) invite link of your choice
- **Per-User Invite** - Infinity List will create an invite for this server for each user
- **None** - This server will not be invitable. Useful, if you wish to use a whitelist form and manually send out invites
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
        .await_component_interaction(ctx.serenity_context().shard.clone())
        .author_id(ctx.author().id)
        .timeout(Duration::from_secs(360))
        .await;

    if let Some(m) = &interaction {
        let id = &m.data.custom_id;

        msg.edit(
            ctx,
            builder
                .to_prefix_edit(serenity::all::EditMessage::new())
                .components(vec![]),
        )
        .await?; // remove buttons after button press

        Ok(match id.as_str() {
            "cancel" => return Err("Setup cancelled".into()),
            "invite_url" => {
                // Ask for invite url now
                let qm = CreateQuickModal::new("Invite URL Selection").field(
                    CreateInputText::new(InputTextStyle::Short, "Enter Invite URL", "invite_url")
                        .placeholder("Please enter the Invite URL you wish to use!")
                        .min_length(20)
                        .max_length(100),
                );

                if let Some(resp) = m.quick_modal(ctx.serenity_context(), qm).await? {
                    let inputs = resp.inputs;

                    let invite_url = &inputs[0];

                    resp.interaction
                        .create_response(
                            ctx.http(),
                            CreateInteractionResponse::Message(
                                CreateInteractionResponseMessage::default().embed(
                                    CreateEmbed::new().title("Resolving invite").description(
                                        format!(
                                            "Please wait while we try to resolve this invite: {}",
                                            invite_url
                                        ),
                                    ),
                                ),
                            ),
                        )
                        .await?;

                    if let Err(e) = resolve_invite(ctx, invite_url).await {
                        resp.interaction
                            .edit_response(
                                ctx.http(),
                                EditInteractionResponse::new().embed(
                                    CreateEmbed::new()
                                        .title("Error resolving invite")
                                        .description(format!(
                                            "This invite could not be resolved: {}",
                                            e
                                        )),
                                ),
                            )
                            .await?;

                        return Err(format!("Error resolving invite: {}", e).into());
                    }

                    resp.interaction
                        .edit_response(
                            ctx.http(),
                            EditInteractionResponse::new().embed(
                                CreateEmbed::new()
                                    .title("Resolved invite successfully!")
                                    .description(format!("You have inputted: {}", invite_url)),
                            ),
                        )
                        .await?;

                    format!("invite_url:{}", invite_url)
                } else {
                    return Err("Timed out waiting for response for invite URL".into());
                }
            }
            "per_user" => {
                // Ask the user to pick a channel
                let qm = CreateQuickModal::new("Per-User Invite Selection")
                    .field(
                        CreateInputText::new(InputTextStyle::Short, "Enter Channel ID", "channel_id")
                            .placeholder("Please enter the Channel ID you wish to use!")
                            .min_length(18)
                            .max_length(18)
                            .required(true),
                    )
                    .field(
                        CreateInputText::new(InputTextStyle::Short, "Max Uses", "max_uses")
                            .placeholder("How many times should a per-user invite be usable for. Use 1 if unsure")
                            .min_length(1)
                            .max_length(3)
                            .required(true),
                    )
                    .field(
                        CreateInputText::new(InputTextStyle::Short, "Max Age", "max_age")
                            .placeholder("How long should the invite be valid for. Use 0 if unsure")
                            .min_length(1)
                            .max_length(3)
                            .required(true)
                    );

                if let Some(resp) = m.quick_modal(ctx.serenity_context(), qm).await? {
                    // Send a please wait response
                    resp.interaction
                        .create_response(
                            ctx.http(),
                            CreateInteractionResponse::Message(
                                CreateInteractionResponseMessage::default().embed(
                                    CreateEmbed::new()
                                        .title("Please wait!")
                                        .description("Please wait..."),
                                ),
                            ),
                        )
                        .await?;

                    let inputs = resp.inputs;

                    let channel_id: serenity::all::ChannelId = inputs[0].parse()?;

                    // Fetch the channel
                    let channel = channel_id.to_channel(ctx).await?;

                    match channel {
                        serenity::all::Channel::Private(_) => {
                            return Err("Channel must be a guild channel".into())
                        }
                        serenity::all::Channel::Guild(c) => {
                            if c.guild_id != guild_id {
                                return Err("Channel must be in this server".into());
                            }
                        }
                        _ => return Err("Channel must be a guild channel".into()),
                    }

                    let max_uses: u8 = inputs[1].parse()?;
                    let max_age: u32 = inputs[2].parse()?;

                    format!("per_user:{}:{}:{}", channel_id, max_uses, max_age)
                } else {
                    return Err("Timed out waiting for response for channel ID".into());
                }
            }
            "none" => "none".to_string(),
            _ => return Err("Invalid choice".into()),
        })
    } else {
        Err("Timed out waiting for choice".into())
    }
}

async fn resolve_invite(ctx: &Context<'_>, invite: &str) -> Result<(), Error> {
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

    let code = url
        .split('/')
        .last()
        .ok_or("Invalid invite URL: No code could be parsed")?;

    let invite = ctx
        .http()
        .get_invite(code, false, true, None)
        .await
        .map_err(|e| format!("Failed to fetch invite: {}", e))?;

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

/// Represents the error that can occur when creating an invite for a user
#[derive(Debug, Serialize, Deserialize, ToSchema, TS, Clone, VariantNames)]
#[ts(export, export_to = ".generated/CreateInviteForUserError.ts")]
pub enum CreateInviteForUserError {
    Generic { message: String },
    ServerNotFound {},
    ServerNeedsLoginForInvite {},
    UserIsBlacklisted {},
    ServerHasNoInvite {},
    ServerHasInvalidInvite {},
    ServerTypeNotApprovedOrCertified {},
    ServerStateNotPublic {},
}

impl core::fmt::Display for CreateInviteForUserError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            CreateInviteForUserError::Generic { message } => write!(f, "{}", message),
            CreateInviteForUserError::ServerNotFound {} => write!(f, "Server not found"),
            CreateInviteForUserError::ServerNeedsLoginForInvite {} => {
                write!(f, "In order to view this server, you must login!")
            }
            CreateInviteForUserError::UserIsBlacklisted {} => {
                write!(f, "User is blacklisted from this server")
            }
            CreateInviteForUserError::ServerHasNoInvite {} => write!(f, "Server has no invite"),
            CreateInviteForUserError::ServerHasInvalidInvite {} => {
                write!(f, "Server has an invalid invite")
            }
            CreateInviteForUserError::ServerTypeNotApprovedOrCertified {} => {
                write!(f, "Server is not approved or certified")
            }
            CreateInviteForUserError::ServerStateNotPublic {} => {
                write!(f, "Server is not public")
            }
        }
    }
}

/// Represents the result of creating an invite for a user
#[derive(Debug, Serialize, Deserialize, ToSchema, TS, Clone, VariantNames)]
#[ts(export, export_to = ".generated/CreateInviteForUserResult.ts")]
pub enum CreateInviteForUserResult {
    Invite { url: String },
}

/// Creates an invite for a user in a guild
///
/// TODO: Improve this with more features if needed (such as whitelist-only servers)
pub async fn create_invite_for_user(
    cache_http: &botox::cache::CacheHttpImpl,
    pool: &sqlx::PgPool,
    guild_id: serenity::all::GuildId,
    user_id: Option<serenity::all::UserId>,
    skip_checks: bool,
) -> Result<CreateInviteForUserResult, CreateInviteForUserError> {
    let row = sqlx::query!(
        "SELECT login_required_for_invite, blacklisted_users, invite, type, state FROM servers WHERE server_id = $1",
        guild_id.to_string()
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        log::error!("Failed to fetch server data: {}", e);
        CreateInviteForUserError::Generic {
            message: format!("Failed to fetch server data: {}", e),
        }
    })?;

    let row = match row {
        Some(r) => r,
        None => return Err(CreateInviteForUserError::ServerNotFound {}),
    };

    if !skip_checks {
        if row.login_required_for_invite {
            let Some(user_id) = user_id else {
                return Err(CreateInviteForUserError::ServerNeedsLoginForInvite {});
            };

            if row.blacklisted_users.contains(&user_id.to_string()) {
                return Err(CreateInviteForUserError::UserIsBlacklisted {});
            }
        }

        if row.r#type != "approved" && row.r#type != "certified" {
            return Err(CreateInviteForUserError::ServerTypeNotApprovedOrCertified {});
        }

        if row.state != "public" {
            return Err(CreateInviteForUserError::ServerStateNotPublic {});
        }
    }

    if row.invite == "none" {
        return Err(CreateInviteForUserError::ServerHasNoInvite {});
    }

    let invite_splitted = row.invite.split(':').collect::<Vec<_>>();

    if invite_splitted.len() < 2 {
        return Err(CreateInviteForUserError::ServerHasInvalidInvite {});
    }

    match invite_splitted[0] {
        "invite_url" => {
            let invite = invite_splitted[1..].join(":");
            Ok(CreateInviteForUserResult::Invite { url: invite })
        }
        "per_user" => {
            let channel_id = invite_splitted[1]
                .parse::<serenity::all::ChannelId>()
                .map_err(|_| CreateInviteForUserError::ServerHasInvalidInvite {})?;
            let max_uses = if invite_splitted.len() >= 3 {
                invite_splitted[2]
                    .parse::<u8>()
                    .map_err(|_| CreateInviteForUserError::ServerHasInvalidInvite {})?
            } else {
                1 // default to 1
            };
            let max_age = if invite_splitted.len() >= 4 {
                invite_splitted[3]
                    .parse::<u32>()
                    .map_err(|_| CreateInviteForUserError::ServerHasInvalidInvite {})?
            } else {
                300 // default to 5 minutes
            };

            let invite = channel_id
                .create_invite(
                    &cache_http.http,
                    serenity::all::CreateInvite::default()
                        .max_uses(max_uses)
                        .max_age(max_age)
                        .unique(true)
                        .audit_log_reason(
                            match user_id {
                                Some(user_id) => format!("Invite created for user {}", user_id),
                                None => "Invite created for anonymous user".to_string(),
                            }
                            .as_str(),
                        ),
                )
                .await
                .map_err(|e| {
                    log::error!("Failed to create invite: {}", e);
                    CreateInviteForUserError::Generic {
                        message: format!("Failed to create invite: {}", e),
                    }
                })?;

            Ok(CreateInviteForUserResult::Invite { url: invite.url() })
        }
        _ => Err(CreateInviteForUserError::ServerHasInvalidInvite {}),
    }
}
