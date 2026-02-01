use solana_client::pubsub_client::PubsubClient;
use solana_sdk::pubkey::Pubkey;
use anchor_lang::prelude::*;
use base64::{engine::general_purpose, Engine as _};
use borsh::BorshDeserialize;
use sha2::{Digest, Sha256};
use std::env;

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

fn event_discriminator(name: &str) -> [u8; 8] {
    let mut hasher = Sha256::new();
    hasher.update(format!("event:{}", name));
    let hash = hasher.finalize();
    let mut disc = [0u8; 8];
    disc.copy_from_slice(&hash[..8]);
    disc
}


pub fn listen(ws_url: String, program_id: Pubkey) {
    let (_client, receiver) =
        PubsubClient::logs_subscribe(
            &ws_url,
            solana_client::rpc_config::RpcTransactionLogsFilter::Mentions(vec![
                program_id.to_string(),
            ]),
            solana_client::rpc_config::RpcTransactionLogsConfig { commitment: None },
        )
        .unwrap();

    println!(
        "Listening Solana events on {} for program {}",
        ws_url, program_id
    );

    for msg in receiver {
        for log in &msg.value.logs {
            if let Some(base64_data) = log.strip_prefix("Program data: ") {
                let Ok(bytes) = general_purpose::STANDARD.decode(base64_data) else {
                    continue;
                };

                if bytes.len() < 8 {
                    continue;
                }

                let (disc, data) = bytes.split_at(8);

                if disc == event_discriminator("OrderCreated") {
                    match OrderCreated::try_from_slice(data) {
                        Ok(e) => println!("🆕 OrderCreated: {:?}", e),
                        Err(err) => eprintln!("Decode OrderCreated failed: {:?}", err),
                    }
                } 
                else if disc == event_discriminator("OrderAccepted") {
                    match OrderAccepted::try_from_slice(data) {
                        Ok(e) => println!("🤝 OrderAccepted: {:?}", e),
                        Err(err) => eprintln!("Decode OrderAccepted failed: {:?}", err),
                    }
                } 
                else if disc == event_discriminator("OrderCompleted") {
                    match OrderCompleted::try_from_slice(data) {
                        Ok(e) => println!("✅ OrderCompleted: {:?}", e),
                        Err(err) => eprintln!("Decode OrderCompleted failed: {:?}", err),
                    }
                }
            }
        }
    }

}

#[tokio::main]
async fn main() {
    // 1️⃣ Program ID
    let program_id_str =
        env::var("PROGRAM_ID").expect("PROGRAM_ID env var not set");

    let program_id: Pubkey = program_id_str
        .parse()
        .expect("Invalid PROGRAM_ID");

    // 2️⃣ WebSocket URL
    let ws_url =
        env::var("WS_URL").unwrap_or_else(|_| "ws://127.0.0.1:8900".to_string());

    println!("PROGRAM_ID = {}", program_id);
    println!("WS_URL     = {}", ws_url);

    std::thread::spawn(move || {
        listen(ws_url, program_id);
    });

    // keep process alive without busy loop
    loop {
        std::thread::park();
    }
}
