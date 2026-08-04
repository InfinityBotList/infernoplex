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
mod shadowclaw;
mod sorbet;
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

/// User data, which is stored and accessible in all command invocations
#[allow(dead_code)]
pub struct Data {
    pool: sqlx::PgPool,
    intents: serenity::all::GatewayIntents,
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
                    // Only real production runs background tasks (server sync,
                    // team cleanup) against real guilds — staging and dev both
                    // skip them.
                    if *crate::config::CURRENT_ENV == crate::config::CURRENT_ENV_PROD {
                        tokio::task::spawn(botox::taskman::start_all_tasks(
                            crate::tasks::tasks(),
                            ctx.serenity_context.clone(),
                        ));
                    }

                    CONNECT_STATE.write().await.has_started_bgtasks = true;
                }
            }
        }
        // Team-member cleanup (removing someone who left the guild or lost
        // Administrator) used to happen here, reacting to
        // GuildMemberUpdate/GuildMemberRemoval. Those events require the
        // privileged Server Members intent, which this bot deliberately
        // doesn't request. See tasks::teamcleanup for the REST-polling
        // replacement, which needs no privileged intent at all.
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

    // gateway.nodebyte.host (ByteProxy) strips whatever Authorization header
    // the caller sends and replaces it with its own shared bot credential,
    // to stop callers relaying arbitrary upstream auth through it. Since
    // Infernoplex needs to authenticate as its own bot application rather
    // than whichever bot ByteProxy is configured with, the token is instead
    // passed via X-Upstream-Authorization, which ByteProxy's `discord`
    // service is opted in (`allowCallerOverride`) to honor and forward as
    // the real Authorization header sent to Discord.
    let mut proxy_headers = reqwest::header::HeaderMap::new();
    proxy_headers.insert(
        "x-upstream-authorization",
        reqwest::header::HeaderValue::from_str(&format!("Bot {}", config::CONFIG.token.get()))
            .expect("Discord token should be a valid header value"),
    );

    let proxy_client = reqwest::Client::builder()
        .use_rustls_tls()
        .default_headers(proxy_headers)
        .build()
        .expect("Failed to build reqwest client for Discord proxy");

    let http = Arc::new(
        serenity::all::HttpBuilder::new(&config::CONFIG.token.get())
            .client(proxy_client)
            .proxy(config::CONFIG.proxy_url.clone())
            .ratelimiter_disabled(true)
            .build(),
    );

    let application_info = http
        .get_current_application_info()
        .await
        .expect("Could not get app info");

    // Deliberately non-privileged only: GUILD_MEMBERS and GUILD_PRESENCES
    // are never requested. Slash-command interactions and guild-level data
    // (roles, channels) don't need them; team-member cleanup and online
    // counts are handled via REST polling instead (see tasks::teamcleanup
    // and shadowclaw::stats). This means infernoplex never needs to apply
    // for privileged-intent verification at all, regardless of scale.
    let intents = serenity::all::GatewayIntents::default();

    let client_builder = serenity::all::ClientBuilder::new_with_http(http, intents);

    let pool = PgPoolOptions::new()
        .max_connections(MAX_CONNECTIONS)
        .connect(&config::CONFIG.database_url)
        .await
        .expect("Could not initialize connection");

    let data = Data {
        pool: pool.clone(),
        intents,
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
            cmds::server::leaderboard::leaderboard(),
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

    // Start sorbet server
    let cache_http_papi = botox::cache::CacheHttpImpl {
        http: client.http.clone(),
        cache: client.cache.clone(),
    };

    tokio::task::spawn(sorbet::server::setup_server(
        pool,
        cache_http_papi,
        intents,
        application_info.id,
    ));

    if let Err(why) = client.start().await {
        error!("Client error: {:?}", why);
    }
}
