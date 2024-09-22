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
use tokio::sync::mpsc;
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

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
pub struct VoiceMessage {
    pub(crate) guild: GuildId,
    pub(crate) event: EventMessage,
}

pub struct WebsocketServer {
    pub event_tx: mpsc::Sender<VoiceMessage>,
}

impl TypeMapKey for WebsocketServer {
    type Value = WebsocketServer;
}

type ConnMap = Arc<DashMap<SocketAddr, mpsc::Sender<VoiceMessage>>>;

impl WebsocketServer {
    pub async fn new() -> Self {
        let listener = TcpListener::bind("0.0.0.0:47336").await.expect("Failed to bind");
        info!("Websocket server listening on 0.0.0.0:47336");

        let (event_tx, event_rx) = mpsc::channel(256);

        let conn_set: ConnMap = Arc::new(DashMap::new());
        let accept_conn_set = conn_set.clone();

        tokio::spawn(async move {
            while let Ok((stream, addr)) = listener.accept().await {
                tokio::spawn(Self::handle_accept(accept_conn_set.clone(), stream, addr));
            }
        });

        tokio::spawn(Self::handle_events(conn_set, event_rx));

        Self {
            event_tx,
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

    async fn handle_accept(conn_map: ConnMap, raw_stream: TcpStream, addr: SocketAddr) {
        let (path_tx, path_rx) = tokio::sync::oneshot::channel();

        let ws_stream = tokio_tungstenite::accept_hdr_async(raw_stream, Self::handle_header(path_tx))
            .await
            .expect("Error during the websocket handshake occurred");
        info!("WebSocket connection established: {}", addr);

        let gid = path_rx.await.unwrap();

        let (conn_tx, mut conn_rx) = mpsc::channel(256);
        debug!("Adding {addr} to the connection map...");
        conn_map.insert(addr, conn_tx.clone());

        let (mut send, mut recv) = ws_stream.split();

        let handle_heartbeat = tokio::spawn(async move {
            let mut heartbeat_interval = interval(Duration::from_secs(10));
            loop {
                heartbeat_interval.tick().await;
                if let Err(_) = conn_tx.send(VoiceMessage { guild: gid, event: EventMessage::Heartbeat }).await {
                    trace!("{addr}: Send fail, bailing from heartbeat loop...");
                    return
                }
            }
        });

        let handle_incoming = tokio::spawn(async move {
            while let Some(Ok(_)) = recv.next().await {}
            trace!("{addr}: Connection dropped, handle_incoming() terminating...");
        });

        let handle_outgoing = tokio::spawn(async move {
            while let Some(msg) = conn_rx.recv().await {
                if msg.guild == gid {
                    trace!("{addr}: Sending to client: {msg:?}");
                    let buf = match serde_json::to_string(&msg.event) {
                        Ok(x) => x,
                        Err(e) => {
                            error!("{addr}: Failed to serialize the message {msg:?}\nError: {e:?}");
                            continue
                        }
                    };
                    let ws_msg = WsMessage::Text(buf);
                    if let Err(e) = send.send(ws_msg).await {
                        error!("{addr}: Failed to send packet to client: {e:?}");
                        return
                    }
                }
            }
        });

        select! {
            _ = handle_incoming => (),
            _ = handle_outgoing => (),
            _ = handle_heartbeat => (),
        }

        debug!("{} disconnected", &addr);
    }

    async fn handle_events(conn_map: ConnMap, mut event_rx: mpsc::Receiver<VoiceMessage>) {
        while let Some(msg) = event_rx.recv().await {
            let mut dead_conns = Vec::new();

            trace!("Got event: {msg:?}");
            for conn in conn_map.iter() {
                trace!("Sending event {msg:?} to channel for {:?}", conn.key());
                if let Err(_)  = conn.send(msg.clone()).await {
                    debug!("Channel for {} failed to send, queueing for removal from conn_map...", conn.key());
                    dead_conns.push(conn.key().clone());
                }
            }

            for dead_conn in dead_conns {
                conn_map.remove(&dead_conn);
            }
        }
    }
}