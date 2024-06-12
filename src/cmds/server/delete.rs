use crate::{Context, Error};
use poise::{
    serenity_prelude::{ButtonStyle, CreateActionRow, CreateButton, CreateEmbed},
    CreateReply,
};
use std::time::Duration;

/// Delete your server from Infinity List, needs 'servers.delete' permissions
#[poise::command(prefix_command, slash_command, required_permissions = "MANAGE_GUILD")]
pub async fn delete(ctx: Context<'_>) -> Result<(), Error> {
    if ctx.guild_id().is_none() {
        ctx.send(
            CreateReply::new().embed(
                CreateEmbed::new()
                    .title("Error!")
                    .description("This command can only be executed in a server!"),
            ),
        )
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

            // Get the servers vanity
            let vanity = sqlx::query!(
                "SELECT vanity_ref FROM servers WHERE server_id = $1",
                server_id
            )
            .fetch_one(&mut *tx)
            .await?;

            // Delete the vanity
            sqlx::query!("DELETE FROM vanity WHERE itag = $1", vanity.vanity_ref)
                .execute(&mut *tx)
                .await?;

            sqlx::query!("DELETE FROM servers WHERE server_id = $1", server_id)
                .execute(&mut *tx)
                .await?;

            tx.commit().await?;

            // Finish interaction.
            ctx.send(
                CreateReply::new().embed(
                    CreateEmbed::new()
                        .title("All Done!")
                        .description("All done :white_check_mark: "),
                ),
            )
            .await?;
            return Ok(());
        }
    }

    ctx.send(
        CreateReply::new().embed(
            CreateEmbed::new()
                .title("Error!")
                .description("This server is not on Infinity List! Run `/setup` to enlist it!"),
        ),
    )
    .await?;
    return Ok(());
}
