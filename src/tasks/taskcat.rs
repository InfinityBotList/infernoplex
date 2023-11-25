use log::{error, info};
use std::time::Duration;
use strum::IntoEnumIterator;
use strum_macros::{Display, EnumIter};
use tokio::task::JoinSet;

#[derive(EnumIter, Display)]
#[strum(serialize_all = "snake_case")]
pub enum Task {
    ServerSync
}

impl Task {
    /// Whether or not the task is enabled
    pub fn enabled(&self) -> bool {
        match self {
            Task::ServerSync => true,
        }
    }

    /// How often the task should run
    pub fn duration(&self) -> Duration {
        match self {
            Task::ServerSync => Duration::from_secs(300),
        }
    }

    /// Description of the task
    pub fn description(&self) -> &'static str {
        match self {
            Task::ServerSync => "Syncing servers",
        }
    }

    /// Function to run the task
    pub async fn run(
        &self,
        pool: &sqlx::PgPool,
        cache_http: &crate::impls::cache::CacheHttpImpl,
    ) -> Result<(), crate::Error> {
        match self {
            Task::ServerSync => crate::tasks::serversync::server_sync(pool, cache_http).await,
        }
    }
}

/// Function to start all tasks
pub async fn start_all_tasks(
    pool: sqlx::PgPool,
    cache_http: crate::impls::cache::CacheHttpImpl,
) -> ! {
    // Start tasks
    let mut set = JoinSet::new();

    for task in Task::iter() {
        if !task.enabled() {
            continue;
        }

        set.spawn(crate::tasks::taskcat::taskcat(
            pool.clone(),
            cache_http.clone(),
            task,
        ));
    }

    if let Some(res) = set.join_next().await {
        if let Err(e) = res {
            error!("Error while running task: {}", e);
        }

        info!("Task finished when it shouldn't have");
        std::process::abort();
    }

    info!("All tasks finished when they shouldn't have");
    std::process::abort();
}

/// Function that manages a task
async fn taskcat(
    pool: sqlx::PgPool,
    cache_http: crate::impls::cache::CacheHttpImpl,
    task: Task,
) -> ! {
    let duration = task.duration();
    let description = task.description();

    // Ensure multiple tx's are not created at the same time
    tokio::time::sleep(duration).await;

    let mut interval = tokio::time::interval(duration);

    loop {
        interval.tick().await;

        log::info!(
            "TASK: {} ({}s interval) [{}]",
            task.to_string(),
            duration.as_secs(),
            description
        );

        if let Err(e) = task.run(&pool, &cache_http).await {
            log::error!("TASK {} ERROR'd: {:?}", task.to_string(), e);
        }
    }
}
