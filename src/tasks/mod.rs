pub mod serversync;

use botox::taskman::Task;
use futures_util::FutureExt;

pub fn tasks() -> Vec<Task> {
    vec![Task {
        name: "serversync",
        description: "Synchronises server data with the database",
        duration: std::time::Duration::from_secs(60),
        enabled: true,
        run: Box::new(move |ctx| crate::tasks::serversync::server_sync(ctx).boxed()),
    }]
}
