use std::time::Duration;

use tokio::task::JoinHandle;

use crate::esi::processor::Job;

pub const REFRESH_INTERVAL: Duration = Duration::from_secs(5 * 60);
pub const FETCH_INTERVAL: Duration = Duration::from_secs(10 * 60);
pub const RESOLVE_INTERVAL: Duration = Duration::from_secs(60 * 60);

pub async fn start_scheduler(
    mut stop: tokio::sync::oneshot::Receiver<()>,
    scheduler_sender: tokio::sync::mpsc::Sender<Job>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut refresh_interval = tokio::time::interval(REFRESH_INTERVAL);
        let mut fetch_interval = tokio::time::interval(FETCH_INTERVAL);
        let mut resolve_interval = tokio::time::interval(RESOLVE_INTERVAL);

        loop {
            tokio::select! {
                _ = refresh_interval.tick() => {
                    tracing::info!("refresh?");
                    let _ = scheduler_sender.send(Job::Refresh).await;
                }
                _ = fetch_interval.tick() => {
                    tracing::info!("fetch?");
                    let _ = scheduler_sender.send(Job::Killmails).await;
                }
                _ = resolve_interval.tick() => {
                    tracing::info!("resolve?");
                    // add refresh here
                }
                _ = &mut stop => {
                    tracing::info!("Scheduler received stop signal, shutting down.");
                    break;
                }
            }
        }
    })
}
