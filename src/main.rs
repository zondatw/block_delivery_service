use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use anchor_lang::prelude::*;
use base64::{engine::general_purpose, Engine as _};
use borsh::BorshDeserialize;
use serde::Serialize;
use sha2::{Digest, Sha256};
use solana_client::{
    pubsub_client::PubsubClient,
    rpc_config::{RpcTransactionLogsConfig, RpcTransactionLogsFilter},
};
use solana_sdk::pubkey::Pubkey;
use std::{env, thread};
use tokio::{net::TcpListener, sync::broadcast};
use tracing::{info, warn};

//
// ---------------- Anchor event structs
//
#[derive(Debug, BorshDeserialize)]
pub struct OrderCreated {
    pub order: Pubkey,
    pub order_id: u64,
    pub customer: Pubkey,
    pub amount: u64,
}

#[derive(Debug, BorshDeserialize)]
pub struct OrderAccepted {
    pub order: Pubkey,
    pub courier: Pubkey,
}

#[derive(Debug, BorshDeserialize)]
pub struct OrderCompleted {
    pub order: Pubkey,
    pub order_id: u64,
    pub courier: Pubkey,
    pub amount: u64,
}

//
// ---------------- Web JSON events
//
#[derive(Debug, Serialize, Clone)]
#[serde(tag = "type")]
pub enum WebEvent {
    OrderCreated {
        order: String,
        order_id: u64,
        customer: String,
        amount: u64,
    },
    OrderAccepted {
        order: String,
        courier: String,
    },
    OrderCompleted {
        order: String,
        order_id: u64,
        courier: String,
        amount: u64,
    },
}

type Tx = broadcast::Sender<WebEvent>;

//
// ---------------- Anchor event discriminator
//
fn event_discriminator(name: &str) -> [u8; 8] {
    let mut hasher = Sha256::new();
    hasher.update(format!("event:{}", name));
    let hash = hasher.finalize();
    let mut disc = [0u8; 8];
    disc.copy_from_slice(&hash[..8]);
    disc
}

//
// ---------------- Solana PubSub listener
//
fn listen(ws_url: String, program_id: Pubkey, tx: Tx) {
    let (_client, receiver) = PubsubClient::logs_subscribe(
        &ws_url,
        RpcTransactionLogsFilter::Mentions(vec![program_id.to_string()]),
        RpcTransactionLogsConfig { commitment: None },
    )
    .expect("logs_subscribe failed");

    info!(
        "📡 Listening Solana events on {} for program {}",
        ws_url, program_id
    );

    for msg in receiver {
        for log in &msg.value.logs {
            let Some(base64_data) = log.strip_prefix("Program data: ") else {
                continue;
            };

            let Ok(bytes) = general_purpose::STANDARD.decode(base64_data) else {
                continue;
            };

            if bytes.len() < 8 {
                continue;
            }

            let (disc, data) = bytes.split_at(8);

            if disc == event_discriminator("OrderCreated") {
                if let Ok(e) = OrderCreated::try_from_slice(data) {
                    let _ = tx.send(WebEvent::OrderCreated {
                        order: e.order.to_string(),
                        order_id: e.order_id,
                        customer: e.customer.to_string(),
                        amount: e.amount,
                    });
                }
            } else if disc == event_discriminator("OrderAccepted") {
                if let Ok(e) = OrderAccepted::try_from_slice(data) {
                    let _ = tx.send(WebEvent::OrderAccepted {
                        order: e.order.to_string(),
                        courier: e.courier.to_string(),
                    });
                }
            } else if disc == event_discriminator("OrderCompleted") {
                if let Ok(e) = OrderCompleted::try_from_slice(data) {
                    let _ = tx.send(WebEvent::OrderCompleted {
                        order: e.order.to_string(),
                        order_id: e.order_id,
                        courier: e.courier.to_string(),
                        amount: e.amount,
                    });
                }
            }
        }
    }
}

//
// ---------------- WebSocket handler
//
async fn ws_handler(
    ws: WebSocketUpgrade,
    State(tx): State<Tx>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, tx))
}

async fn handle_socket(mut socket: WebSocket, tx: Tx) {
    let mut rx = tx.subscribe();

    info!("🌐 Web client connected");

    while let Ok(event) = rx.recv().await {
        let Ok(json) = serde_json::to_string(&event) else {
            continue;
        };

        if socket.send(Message::Text(json)).await.is_err() {
            warn!("❌ Web client disconnected");
            break;
        }
    }
}

//
// ---------------- HTTP / WS server
//
async fn start_server() -> Tx {
    let (tx, _) = broadcast::channel(100);

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .with_state(tx.clone());

    tokio::spawn(async move {
        let listener = TcpListener::bind("0.0.0.0:3000")
            .await
            .expect("bind failed");

        info!("🚀 WebSocket server on ws://localhost:3000/ws");

        axum::serve(listener, app)
            .await
            .expect("server failed");
    });

    tx
}

//
// ---------------- main
//
#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let program_id: Pubkey = env::var("PROGRAM_ID")
        .expect("PROGRAM_ID not set")
        .parse()
        .expect("Invalid PROGRAM_ID");

    let ws_url =
        env::var("WS_URL").unwrap_or_else(|_| "ws://127.0.0.1:8900".to_string());

    let tx = start_server().await;

    thread::spawn(move || {
        listen(ws_url, program_id, tx);
    });

    loop {
        thread::park();
    }
}
