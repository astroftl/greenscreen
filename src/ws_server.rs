use std::net::SocketAddr;
use std::num::ParseIntError;
use std::str::FromStr;
use std::sync::Arc;
use dashmap::DashMap;
use dashmap::mapref::one::Ref;
use serde::{Deserialize, Serialize};
use serenity::model::id::{GuildId, UserId};
use serenity::futures::{SinkExt, StreamExt};
use serenity::prelude::TypeMapKey;
use tokio::net::{TcpListener, TcpStream};
use tokio::select;
use tokio::sync::broadcast::{channel, Sender};
use tokio_tungstenite::tungstenite::Message as WsMessage;
use tungstenite::handshake::server::{ErrorResponse, Request, Response};

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Message {
    Connected(UserId),
    Speaking(UserId),
    Quiet(UserId),
    Disconnected(UserId)
}

pub struct WebsocketServer {
    pub tx: Arc<DashMap<GuildId, Sender<Message>>>,
}

impl TypeMapKey for WebsocketServer {
    type Value = WebsocketServer;
}

impl WebsocketServer {
    pub async fn new() -> Self {
        let tx = Arc::new(DashMap::new());

        let listener = TcpListener::bind("0.0.0.0:47336").await.expect("Failed to bind");
        info!("Websocket server listening on 0.0.0.0:47336");

        let ret = Self {
            tx: tx.clone(),
        };

        tokio::spawn(async move {
            while let Ok((stream, addr)) = listener.accept().await {
                tokio::spawn(Self::handle_accept(tx.clone(), stream, addr));
            }
        });

        ret
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

    async fn handle_accept(tx: Arc<DashMap<GuildId, Sender<Message>>>, raw_stream: TcpStream, addr: SocketAddr) {
        let (path_tx, path_rx) = tokio::sync::oneshot::channel();

        let ws_stream = tokio_tungstenite::accept_hdr_async(raw_stream, Self::handle_header(path_tx))
            .await
            .expect("Error during the websocket handshake occurred");
        info!("WebSocket connection established: {}", addr);

        let gid = path_rx.await.unwrap();

        let conn_tx = tx.get(&gid);

        let mut conn_rx = match conn_tx {
            Some(conn_tx) => {
                conn_tx.subscribe()
            }
            None => {
                let (conn_tx, conn_rx) = channel(96);
                tx.insert(gid, conn_tx);
                conn_rx
            }
        };

        let (mut send, mut recv) = ws_stream.split();

        let handle_incoming = tokio::spawn(async move {
            while let Some(Ok(_)) = recv.next().await {}
        });

        let handle_outgoing = tokio::spawn(async move {
            while let Ok(msg) = conn_rx.recv().await {
                debug!("Sending to client: {msg:?}");
                let buf = serde_json::to_string(&msg).unwrap();
                let ws_msg = WsMessage::Text(buf);
                send.send(ws_msg).await.unwrap();
            }
        });

        select! {
            _ = handle_incoming => (),
            _ = handle_outgoing => ()
        }

        debug!("{} disconnected", &addr);
    }
}