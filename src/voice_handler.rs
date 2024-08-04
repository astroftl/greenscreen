use std::collections::HashSet;
use std::sync::Arc;
use dashmap::DashMap;
use serenity::all::GuildId;
use serenity::async_trait;
use serenity::model::voice_gateway::payload::{ClientDisconnect, Speaking};
use serenity::model::id::UserId;
use songbird::{EventContext, EventHandler};
use tokio::sync::Mutex;
use crate::ws_server::{Message, WebsocketServer};

#[derive(Clone)]
pub(crate) struct Receiver {
    inner: Arc<InnerReceiver>,
}

struct InnerReceiver {
    guild_id: GuildId,
    known_ssrcs: DashMap<u32, UserId>,
    last_talking: Mutex<HashSet<UserId>>,
    websocket: Arc<WebsocketServer>,
}

impl Receiver {
    pub async fn new(guild_id: GuildId, ws_serv: Arc<WebsocketServer>) -> Self {
        Self {
            inner: Arc::new(InnerReceiver {
                guild_id,
                known_ssrcs: DashMap::new(),
                last_talking: Mutex::new(HashSet::new()),
                websocket: ws_serv,
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

                        if let Some(send_tx) = self.inner.websocket.tx.get(&self.inner.guild_id) {
                            let send_res = send_tx.send(Message::Connected(user));
                            if let Err(e) = send_res {
                                error!("Failed to send message: {e}")
                            }
                        }
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
                    if let Some(send_tx) = self.inner.websocket.tx.get(&self.inner.guild_id) {
                        let send_res = send_tx.send(Message::Speaking(*user));
                        if let Err(e) = send_res {
                            error!("Failed to send message: {e}")
                        }
                    }
                }

                for user in new_quiet {
                    if let Some(send_tx) = self.inner.websocket.tx.get(&self.inner.guild_id) {
                        let send_res = send_tx.send(Message::Quiet(*user));
                        if let Err(e) = send_res {
                            error!("Failed to send message: {e}")
                        }
                    }
                }

                *last_talking = current_talking;
            },
            Ctx::ClientDisconnect(ClientDisconnect { user_id, .. }) => {
                debug!("Client disconnected: user {:?}", user_id);
                if let Some(send_tx) = self.inner.websocket.tx.get(&self.inner.guild_id) {
                    let send_res = send_tx.send(Message::Disconnected(UserId::from(user_id.0)));
                    if let Err(e) = send_res {
                        error!("Failed to send message: {e}")
                    }
                }
            },
            _ => {
                unimplemented!()
            },
        }

        None
    }
}