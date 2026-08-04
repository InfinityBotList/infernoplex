pub mod serversync;
pub mod teamcleanup;

use botox::taskman::Task;
use futures_util::FutureExt;

pub fn tasks() -> Vec<Task> {
    vec![
        Task {
            name: "serversync",
            description: "Syncs opted-in servers' emojis/stickers into the database",
            duration: std::time::Duration::from_secs(30 * 60),
            enabled: true,
            run: Box::new(move |ctx| crate::tasks::serversync::server_sync(ctx).boxed()),
        },
        Task {
            name: "teamcleanup",
            description: "Removes team members who've left their server or lost Administrator, via REST polling instead of the privileged Server Members intent",
            duration: std::time::Duration::from_secs(15 * 60),
            enabled: true,
            run: Box::new(move |ctx| crate::tasks::teamcleanup::team_cleanup(ctx).boxed()),
        },
    ]
}
