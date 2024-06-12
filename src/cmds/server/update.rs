use crate::{Context, Error};
use poise::{
    serenity_prelude::{
        ButtonStyle, CreateActionRow, CreateButton, CreateEmbed, CreateInputText,
        CreateInteractionResponse, CreateInteractionResponseMessage, CreateQuickModal,
        InputTextStyle,
    },
    CreateReply,
};
use std::time::Duration;

async fn _update_check(ctx: Context<'_>) -> Result<bool, Error> {
    crate::splashtail::perms::check_for_permission(&ctx, "server.edit").await?;
    Ok(true)
}

#[derive(poise::ChoiceParameter)]
enum UpdatePane {
    #[name = "Basic Server Information"]
    BasicInfo,
    #[name = "Server Invite"]
    Invite,
}

/// Update your server information on Infinity List, needs 'server.edit' permissions
#[poise::command(prefix_command, slash_command, check = "_update_check")]
pub async fn update(
    ctx: Context<'_>,
    #[description = "The pane to update"] pane: UpdatePane,
) -> Result<(), Error> {
    let Some(guild_id) = ctx.guild_id() else {
        return Err("This command can only be executed in a server".into());
    };

    match pane {
        UpdatePane::BasicInfo => {
            // Create a button with next+cancel options
            let builder = CreateReply::default()
            .embed(
                CreateEmbed::new()
                    .title("Update Server Information")
                    .description("Oh, hello there :eyes:\nI see you want to update your server listing on Infinity List! Let's get started!")
            )
            .components(vec![
                CreateActionRow::Buttons(vec![
                    CreateButton::new("next")
                        .label("Next")
                        .style(ButtonStyle::Primary),
                    CreateButton::new("cancel")
                        .label("Cancel")
                        .style(ButtonStyle::Danger)
                ])
            ]);

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
                    CreateReply::default()
                        .to_prefix_edit(serenity::all::EditMessage::default())
                        .components(vec![]),
                )
                .await?;

                if id == "cancel" {
                    return Ok(());
                }

                // Create quick modal asking for short and long descriptions
                let qm = CreateQuickModal::new("Update Server Information")
                    .field(
                        CreateInputText::new(InputTextStyle::Short, "Short Description", "short")
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
                                        .title("Updating server...")
                                        .description("This may take a second, please wait..."),
                                ),
                            ),
                        )
                        .await?;

                    // Update the server information in the database
                    sqlx::query!(
                        "UPDATE servers SET short = $2, long = $3 WHERE server_id = $1",
                        guild_id.to_string(),
                        &inputs[1].to_string(),
                        &inputs[2].to_string(),
                    )
                    .execute(&ctx.data().pool)
                    .await?;

                    // Confirm the update to the user
                    ctx.send(
                        CreateReply::new().embed(
                            CreateEmbed::new()
                                .title("All Done!")
                                .description("All done :white_check_mark: "),
                        ),
                    )
                    .await?;
                } else {
                    ctx.send(
                        CreateReply::new()
                            .embed(
                                CreateEmbed::new()
                                    .title("Modal Timed Out")
                                    .description("Please rerun `/update`!"),
                            )
                            .ephemeral(true),
                    )
                    .await?;
                }
            } else {
                ctx.send(
                    CreateReply::new()
                        .embed(
                            CreateEmbed::new()
                                .title("Update Timed Out")
                                .description("Please rerun `/update`!"),
                        )
                        .ephemeral(true),
                )
                .await?;
            }
        }
        UpdatePane::Invite => {
            let invite = crate::splashtail::invite::setup_invite_view(&ctx).await?;

            // Save to the database
            sqlx::query!(
                "UPDATE servers SET invite = $2 WHERE server_id = $1",
                guild_id.to_string(),
                invite
            )
            .execute(&ctx.data().pool)
            .await?;

            ctx.send(
                CreateReply::new().embed(
                    CreateEmbed::new()
                        .title("All Done!")
                        .description("All done :white_check_mark:"),
                ),
            )
            .await?;
        }
    }

    Ok(())
}
