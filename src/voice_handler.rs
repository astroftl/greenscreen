use std::collections::HashSet;
use std::sync::Arc;
use dashmap::DashMap;
use serenity::async_trait;
use serenity::model::voice_gateway::payload::{ClientDisconnect, Speaking};
use serenity::model::id::UserId;
use songbird::{EventContext, EventHandler};
use tokio::sync::broadcast::Sender;
use tokio::sync::Mutex;
use crate::ws_server::EventMessage;

#[derive(Clone)]
pub(crate) struct Receiver {
    inner: Arc<InnerReceiver>,
}

struct InnerReceiver {
    known_ssrcs: DashMap<u32, UserId>,
    last_talking: Mutex<HashSet<UserId>>,
    event_tx: Sender<EventMessage>,
}

impl Receiver {
    pub async fn new(event_tx: Sender<EventMessage>) -> Self {
        Self {
            inner: Arc::new(InnerReceiver {
                known_ssrcs: DashMap::new(),
                last_talking: Mutex::new(HashSet::new()),
                event_tx,
            }),
        }
    }
}

#[async_trait]
impl EventHandler for Receiver {
    #[allow(unused_variables)]
    async fn act(&self, ctx: &EventContext<'_>) -> Option<songbird::Event> {
        use EventContext as Ctx;
        match ctx {
            Ctx::SpeakingStateUpdate(Speaking { speaking, ssrc, user_id, .. }) => {
                if let Some(user) = user_id {
                    let user = UserId::from(user.0);
                    let old_ssrc = self.inner.known_ssrcs.insert(*ssrc, user);
                    if old_ssrc.is_none() {
                        debug!("Speaking state update: user {user_id:?} has SSRC {ssrc:?}, using {speaking:?}");

                        // Ignore the error if there is one, it is not fatal and will occur frequently.
                        _ = self.inner.event_tx.send(EventMessage::Connected(user));
                    }
                }
            },
            Ctx::VoiceTick(tick) => {
                let speaking = tick.speaking.len();
                let total_participants = speaking + tick.silent.len();

                let mut current_talking = HashSet::new();

                for (ssrc, data) in &tick.speaking {
                    if let Some(id) = self.inner.known_ssrcs.get(ssrc) {
                        current_talking.insert(*id);
                    };
                }

                let mut last_talking = self.inner.last_talking.lock().await;

                let new_talking: Vec<_> = current_talking.difference(&last_talking).collect();
                let new_quiet: Vec<_> = last_talking.difference(&current_talking).collect();

                for user in new_talking {
                    _ = self.inner.event_tx.send(EventMessage::Speaking(*user));
                }

                for user in new_quiet {
                    _ = self.inner.event_tx.send(EventMessage::Quiet(*user));
                }

                *last_talking = current_talking;
            },
            Ctx::ClientDisconnect(ClientDisconnect { user_id, .. }) => {
                debug!("Client disconnected: user {:?}", user_id);
                _ = self.inner.event_tx.send(EventMessage::Disconnected(UserId::from(user_id.0)));
            },
            _ => {
                unimplemented!()
            },
        }

        None
    }
}