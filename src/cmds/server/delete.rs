use crate::{Context, Error};
use poise::{
    serenity_prelude::{ButtonStyle, CreateActionRow, CreateButton, CreateEmbed},
    CreateReply,
};
use std::time::Duration;

async fn _delete_check(ctx: Context<'_>) -> Result<bool, Error> {
    crate::splashtail::perms::check_for_permission(&ctx, "server.delete").await?;
    Ok(true)
}

/// Delete your server from Infinity List, needs 'servers.delete' permissions
#[poise::command(prefix_command, slash_command, check = "_delete_check")]
pub async fn delete(ctx: Context<'_>) -> Result<(), Error> {
    let Some(guild_id) = ctx.guild_id() else {
        return Err("This command can only be executed in a server".into());
    };

    // Create button with confirm+cancel
    let builder = CreateReply::default()
            .embed(
                CreateEmbed::new()
                    .title("Confirm Server Deletion?")
                    .description(
                        "Are you sure you want to delete your server from Infinity List? This action is irreversible so think before acting!.",
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
            guild_id.to_string()
        )
        .fetch_one(&mut *tx)
        .await?;

        // Delete the vanity
        sqlx::query!("DELETE FROM vanity WHERE itag = $1", vanity.vanity_ref)
            .execute(&mut *tx)
            .await?;

        // Delete the server
        sqlx::query!(
            "DELETE FROM servers WHERE server_id = $1",
            guild_id.to_string()
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        // Finish interaction.
        ctx.send(
            CreateReply::new().embed(
                CreateEmbed::new()
                    .title("All Done!")
                    .description("All done :white_check_mark:"),
            ),
        )
        .await?;
    }

    Ok(())
}
