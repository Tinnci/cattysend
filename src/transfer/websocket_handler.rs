use crate::transfer::WsMessage;
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::{accept_async, connect_async, tungstenite::Message};
use uuid::Uuid;

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
                let ws_msg: WsMessage = serde_json::from_str(text)?;
                println!("Received WS message: {:?}", ws_msg.msg_type);

                if ws_msg.msg_type == "versionNegotiation" {
                    let resp = WsMessage {
                        msg_type: "versionNegotiation".to_string(),
                        msg_id: Uuid::new_v4().to_string(),
                        data: serde_json::json!({ "version": "1.0" }),
                    };
                    write
                        .send(Message::Text(serde_json::to_string(&resp)?))
                        .await?;
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

        let neg = WsMessage {
            msg_type: "versionNegotiation".to_string(),
            msg_id: Uuid::new_v4().to_string(),
            data: serde_json::json!({ "version": "1.0" }),
        };
        write
            .send(Message::Text(serde_json::to_string(&neg)?))
            .await?;

        if let Some(msg) = read.next().await {
            let msg = msg?;
            println!("Handshake response: {:?}", msg);
        }

        Ok(())
    }
}
