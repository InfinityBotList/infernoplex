use log::{error, info};
use once_cell::sync::Lazy;
use serenity::all::FullEvent;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tokio::sync::RwLock;

mod checks;
mod cmds;
mod config;
mod help;
mod splashtail;
mod stats;
mod tasks;

type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;

pub struct ConnectState {
    pub has_started_bgtasks: bool,
}

pub static CONNECT_STATE: Lazy<RwLock<ConnectState>> = Lazy::new(|| {
    RwLock::new(ConnectState {
        has_started_bgtasks: false,
    })
});

// User data, which is stored and accessible in all command invocations
pub struct Data {
    pool: sqlx::PgPool,
}

#[poise::command(prefix_command)]
async fn register(ctx: Context<'_>) -> Result<(), Error> {
    poise::builtins::register_application_commands_buttons(ctx).await?;
    Ok(())
}

async fn on_error(error: poise::FrameworkError<'_, Data, Error>) {
    // This is our custom error handler
    // They are many errors that can occur, so we only handle the ones we want to customize
    // and forward the rest to the default handler
    match error {
        poise::FrameworkError::Command { error, ctx, .. } => {
            error!("Error in command `{}`: {:?}", ctx.command().name, error,);
            let err = ctx
                .send(
                    poise::CreateReply::new().embed(
                        serenity::all::CreateEmbed::new()
                            .title("Whoa There!")
                            .description(error.to_string()),
                    ),
                )
                .await;

            if let Err(e) = err {
                error!("on_error returned error: {}", e);
            }
        }
        poise::FrameworkError::CommandCheckFailed { error, ctx, .. } => {
            error!(
                "[Possible] error in command `{}`: {:?}",
                ctx.command().name,
                error,
            );
            if let Some(error) = error {
                error!("Error in command `{}`: {:?}", ctx.command().name, error,);
                let err = ctx.say(format!("**{}**", error)).await;

                if let Err(e) = err {
                    error!("Error while sending error message: {}", e);
                }
            }
        }
        error => {
            if let Err(e) = poise::builtins::on_error(error).await {
                error!("Error while handling error: {}", e);
            }
        }
    }
}

async fn event_listener<'a>(
    ctx: poise::FrameworkContext<'a, Data, Error>,
    event: &FullEvent,
) -> Result<(), Error> {
    match event {
        FullEvent::InteractionCreate { interaction } => {
            info!("Interaction received: {:?}", interaction.id());
        }
        FullEvent::Ready { data_about_bot } => {
            info!("{} is ready!", data_about_bot.user.name);

            #[allow(clippy::collapsible_if)]
            if ctx.serenity_context.shard_id.0 == 0 {
                if !CONNECT_STATE.read().await.has_started_bgtasks {
                    if *crate::config::CURRENT_ENV != "staging" {
                        tokio::task::spawn(botox::taskman::start_all_tasks(
                            crate::tasks::tasks(),
                            ctx.serenity_context.clone(),
                        ));
                    }

                    CONNECT_STATE.write().await.has_started_bgtasks = true;
                }
            }
        }
        FullEvent::GuildMemberUpdate { new, .. } => {
            let Some(member) = new else {
                return Err("GuildMemberUpdate: Member not found".into());
            };

            if member.user.bot() {
                return Ok(());
            }

            let pool = &ctx.user_data().pool;

            let permissions = member.permissions(&ctx.serenity_context.cache)?;

            if !permissions.administrator() {
                // Delete them if service is infernoplex
                let res = sqlx::query!(
                    "SELECT team_owner FROM servers WHERE server_id = $1",
                    member.guild_id.to_string(),
                )
                .fetch_optional(pool)
                .await?;

                let team_owner = match res {
                    Some(row) => row.team_owner,
                    None => return Ok(()),
                };

                // Delete them if added_by is infernoplex using a delete statement
                sqlx::query!(
                    "DELETE FROM team_members WHERE team_id = $1 AND user_id = $2 AND service = 'infernoplex'",
                    team_owner,
                    member.user.id.to_string(),
                )
                .execute(pool)
                .await?;
            }
        }
        FullEvent::GuildMemberRemoval { guild_id, user, .. } => {
            // Check the team the server is on, delete them if service is infernoplex
            let pool = &ctx.user_data().pool;

            let res = sqlx::query!(
                "SELECT team_owner FROM servers WHERE server_id = $1",
                guild_id.to_string(),
            )
            .fetch_optional(pool)
            .await?;

            let team_owner = match res {
                Some(row) => row.team_owner,
                None => return Ok(()),
            };

            // Delete them if added_by is infernoplex using a delete statement
            sqlx::query!(
                "DELETE FROM team_members WHERE team_id = $1 AND user_id = $2 AND service = 'infernoplex'",
                team_owner,
                user.id.to_string(),
            )
            .execute(pool)
            .await?;
        }
        _ => {}
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    const MAX_CONNECTIONS: u32 = 3; // max connections to the database, we don't need too many here

    std::env::set_var("RUST_LOG", "infernoplex=info");

    env_logger::init();

    info!("Proxy URL: {}", config::CONFIG.proxy_url);

    let http = Arc::new(
        serenity::all::HttpBuilder::new(&config::CONFIG.token.get())
            .proxy(config::CONFIG.proxy_url.clone())
            .ratelimiter_disabled(true)
            .build(),
    );

    let client_builder = serenity::all::ClientBuilder::new_with_http(
        http,
        serenity::all::GatewayIntents::default()
            | serenity::all::GatewayIntents::GUILD_MEMBERS
            | serenity::all::GatewayIntents::GUILD_PRESENCES,
    );

    let data = Data {
        pool: PgPoolOptions::new()
            .max_connections(MAX_CONNECTIONS)
            .connect(&config::CONFIG.database_url)
            .await
            .expect("Could not initialize connection"),
    };

    let prefix = crate::config::CONFIG.prefix.get();

    let framework = poise::Framework::new(poise::FrameworkOptions {
        initialize_owners: true,
        prefix_options: poise::PrefixFrameworkOptions {
            prefix: Some(prefix.into()),
            ..poise::PrefixFrameworkOptions::default()
        },
        event_handler: |ctx, event| Box::pin(event_listener(ctx, event)),
        commands: vec![
            // Default
            register(),
            help::help(),
            stats::stats(),
            // Custom
            cmds::server::setup::setup(),
            cmds::server::update::update(),
            cmds::server::delete::delete(),
        ],
        // This code is run before every command
        pre_command: |ctx| {
            Box::pin(async move {
                info!(
                    "Executing command {} for user {} ({})...",
                    ctx.command().qualified_name,
                    ctx.author().name,
                    ctx.author().id
                );
            })
        },
        // This code is run after every command returns Ok
        post_command: |ctx| {
            Box::pin(async move {
                info!(
                    "Done executing command {} for user {} ({})...",
                    ctx.command().qualified_name,
                    ctx.author().name,
                    ctx.author().id
                );
            })
        },
        on_error: |error| Box::pin(on_error(error)),
        ..Default::default()
    });

    let mut client = client_builder
        .framework(framework)
        .data(Arc::new(data))
        .await
        .expect("Error creating client");

    if let Err(why) = client.start().await {
        error!("Client error: {:?}", why);
    }
}
