//! WebSocket 处理器（旧版本，保留兼容性）
//!
//! 注意：新代码应使用 transfer::protocol::WsMessage

use crate::transfer::protocol::WsMessage;
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::{accept_async, connect_async, tungstenite::Message};

pub struct WsServer {
    listener: TcpListener,
}

impl WsServer {
    pub async fn bind(addr: &str) -> anyhow::Result<Self> {
        let listener = TcpListener::bind(addr).await?;
        Ok(Self { listener })
    }

    pub async fn accept_one(&self) -> anyhow::Result<()> {
        let (stream, _) = self.listener.accept().await?;
        let ws_stream = accept_async(stream).await?;
        let (mut write, mut read) = ws_stream.split();

        while let Some(msg) = read.next().await {
            let msg = msg?;
            if msg.is_text() {
                let text = msg.to_text()?;
                if let Some(ws_msg) = WsMessage::parse(text) {
                    println!("Received WS message: {:?}", ws_msg.name);

                    if ws_msg.name == "versionNegotiation" {
                        let resp = WsMessage::ack(
                            ws_msg.id,
                            "versionNegotiation",
                            Some(serde_json::json!({ "version": 1 })),
                        );
                        write.send(Message::Text(resp.to_string())).await?;
                    }
                }
            }
        }
        Ok(())
    }
}

pub struct WsClient {
    url: String,
}

impl WsClient {
    pub fn new(url: &str) -> Self {
        Self {
            url: url.to_string(),
        }
    }

    pub async fn connect_and_negotiate(&self) -> anyhow::Result<()> {
        let (ws_stream, _) = connect_async(&self.url).await?;
        let (mut write, mut read) = ws_stream.split();

        let neg = WsMessage::version_negotiation(0);
        write.send(Message::Text(neg.to_string())).await?;

        if let Some(msg) = read.next().await {
            let msg = msg?;
            println!("Handshake response: {:?}", msg);
        }

        Ok(())
    }
}
