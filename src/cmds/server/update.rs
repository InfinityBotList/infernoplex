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

/// Update your server information on Infinity List, needs 'Manage Server' permissions
#[poise::command(prefix_command, slash_command, required_permissions = "MANAGE_GUILD")]
pub async fn update(ctx: Context<'_>) -> Result<(), Error> {
    if let Some(guild_id) = ctx.guild_id() {
        let server_id = guild_id.to_string();

        // Check if the server is already setup
        let res = sqlx::query!(
            "SELECT COUNT(*) as count FROM servers WHERE server_id = $1",
            server_id
        )
        .fetch_one(&ctx.data().pool)
        .await?;

        if res.count.unwrap_or(0) > 0 {
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
                        server_id,
                        &inputs[1].to_string(),
                        &inputs[2].to_string(),
                    )
                    .execute(&ctx.data().pool)
                    .await?;

                    // Confirm the update to the user
                    ctx.say("Server has been successfully updated!").await?;
                    return Ok(());
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
                    return Ok(());
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
                return Ok(());
            }
        } else {
            ctx.say(
                "This server isn't listed on Infinity List. Run `/setup`, if you wish to list it.",
            )
            .await?;
            return Ok(());
        }
    } else {
        ctx.say("This command can only be used in a server.")
            .await?;
        return Ok(());
    }
}
