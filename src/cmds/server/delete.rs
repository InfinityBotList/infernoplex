use crate::{Context, Error};
use poise::{
    serenity_prelude::{ButtonStyle, CreateActionRow, CreateButton, CreateEmbed},
    CreateReply,
};
use std::time::Duration;

/// Delete your server from Infinity List, needs 'Manage Server' permissions
#[poise::command(prefix_command, slash_command, required_permissions = "MANAGE_GUILD")]
pub async fn delete(ctx: Context<'_>) -> Result<(), Error> {
    if ctx.guild_id().is_none() {
        ctx.say("This command can only be used in a server.")
            .await?;
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
        // Create button with confirm+cancel
        let builder = CreateReply::default()
            .embed(
                CreateEmbed::new()
                    .title("Confirm Server Deletion?")
                    .description(
                        "Delete your server from Infinity List! This action cannot be reversed.",
                    ),
            )
            .components(vec![CreateActionRow::Buttons(vec![
                CreateButton::new("confirm")
                    .label("Confirm")
                    .style(ButtonStyle::Danger),
                CreateButton::new("cancel")
                    .label("Cancel")
                    .style(ButtonStyle::Secondary),
            ])]);

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

            // Delete Server
            let mut tx = ctx.data().pool.begin().await?;

            sqlx::query!("DELETE FROM servers WHERE server_id = $1", server_id)
                .execute(&mut *tx)
                .await?;

            tx.commit().await?;

            // Finish interaction.
            ctx.say("Server has been successfully deleted from Infinity List.")
                .await?;
            return Ok(());
        }
    }

    ctx.say("This server isn't listed on Infinity List. Run `/setup`, if you wish to list it.")
        .await?;
    return Ok(());
}
