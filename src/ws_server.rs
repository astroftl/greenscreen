use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use serenity::model::id::{GuildId, UserId};
use serenity::futures::{SinkExt, StreamExt};
use serenity::prelude::TypeMapKey;
use tokio::net::{TcpListener, TcpStream};
use tokio::select;
use tokio::sync::broadcast::{channel, Sender};
use tokio::time::interval;
use tokio_tungstenite::tungstenite::Message as WsMessage;
use tungstenite::handshake::server::{ErrorResponse, Request, Response};

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
pub enum EventMessage {
    Connected(UserId),
    Speaking(UserId),
    Quiet(UserId),
    Disconnected(UserId),
    Heartbeat,
}

pub struct WebsocketServer {
    pub guild_map: GuildMap,
}

impl TypeMapKey for WebsocketServer {
    type Value = WebsocketServer;
}

impl WebsocketServer {
    pub async fn new() -> Self {
        let guild_map = GuildMap::new();

        let listener = TcpListener::bind("0.0.0.0:47336").await.expect("Failed to bind");
        info!("Websocket server listening on 0.0.0.0:47336");

        let accept_guild_map = guild_map.clone();
        tokio::spawn(async move {
            while let Ok((stream, addr)) = listener.accept().await {
                tokio::spawn(Self::handle_accept(accept_guild_map.clone(), stream, addr));
            }
        });

        tokio::spawn(Self::handle_heartbeat(guild_map.clone()));

        Self {
            guild_map,
        }
    }

    async fn handle_heartbeat(guild_map: GuildMap) {
        let mut heartbeat_interval = interval(Duration::from_secs(10));
        loop {
            heartbeat_interval.tick().await;

            for it in guild_map.guild_map.iter() {
                _ = it.send(EventMessage::Heartbeat);
            }
        }
    }

    fn handle_header(sender: tokio::sync::oneshot::Sender<GuildId>) -> impl FnOnce(&Request, Response) -> Result<Response, ErrorResponse> {
        move |req: &Request, res: Response| {
            let path = req.uri().path().trim_start_matches('/');
            trace!("Received a new Websocket handshake at {}", path);

            match GuildId::from_str(path) {
                Ok(guild_id) => {
                    debug!("Received Websocket connection for: {guild_id:?}");
                    sender.send(guild_id).unwrap();
                    Ok(res)
                }
                Err(e) => {
                    error!("Bad Websocket connection: {e}");
                    Err(ErrorResponse::new(Some("Failed to convert path to GuildID".to_string())))
                }
            }
        }
    }

    async fn handle_accept(guild_map: GuildMap, raw_stream: TcpStream, addr: SocketAddr) {
        let (path_tx, path_rx) = tokio::sync::oneshot::channel();

        let ws_stream = tokio_tungstenite::accept_hdr_async(raw_stream, Self::handle_header(path_tx))
            .await
            .expect("Error during the websocket handshake occurred");
        info!("WebSocket connection established: {}", addr);

        let gid = path_rx.await.unwrap();

        let mut conn_rx = guild_map.get(gid).subscribe();

        let (mut send, mut recv) = ws_stream.split();

        let handle_incoming = tokio::spawn(async move {
            while let Some(Ok(_)) = recv.next().await {}
            trace!("Connection dropped, handle_incoming() terminating...");
        });

        let handle_outgoing = tokio::spawn(async move {
            while let Ok(msg) = conn_rx.recv().await {
                trace!("Sending to client: {msg:?}");
                let buf = match serde_json::to_string(&msg) {
                    Ok(x) => x,
                    Err(e) => {
                        error!("Failed to serialize the message {msg:?}\nError: {e:?}");
                        continue
                    }
                };
                let ws_msg = WsMessage::Text(buf);
                if let Err(e) = send.send(ws_msg).await {
                    error!("Failed to send packet to client: {e:?}");
                    return
                }
            }
        });

        select! {
            _ = handle_incoming => (),
            _ = handle_outgoing => ()
        }

        debug!("{} disconnected", &addr);
    }
}

#[derive(Clone, Debug)]
pub struct GuildMap {
    pub guild_map: Arc<DashMap<GuildId, Sender<EventMessage>>>,
}

impl GuildMap {
    pub fn new() -> Self {
        Self {
            guild_map: Arc::new(DashMap::new()),
        }
    }

    pub fn get(&self, guild_id: GuildId) -> Sender<EventMessage> {
        match self.guild_map.get(&guild_id) {
            Some(x) => x.clone(),
            None => {
                let (conn_tx, _) = channel(96);
                self.guild_map.insert(guild_id, conn_tx.clone());
                conn_tx
            }
        }
    }
}