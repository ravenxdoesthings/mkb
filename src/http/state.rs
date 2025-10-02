use crate::esi;

#[derive(Clone)]
pub struct AppState {
    pub jobs_sender: tokio::sync::mpsc::Sender<esi::processor::Job>,
    pub client: esi::EsiClient,
}

impl AppState {
    pub fn new(
        jobs_sender: tokio::sync::mpsc::Sender<esi::processor::Job>,
        client: &esi::EsiClient,
    ) -> Self {
        Self {
            jobs_sender,
            client: client.clone(),
        }
    }
}
