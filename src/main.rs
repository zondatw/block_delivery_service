use solana_client::pubsub_client::PubsubClient;
use solana_sdk::pubkey::Pubkey;
use anchor_lang::prelude::*;
use base64::{engine::general_purpose, Engine as _};
use borsh::BorshDeserialize;

#[derive(Debug, BorshDeserialize)]
pub struct OrderCreated {
    pub order: Pubkey,
    pub amount: u64,
}


pub fn listen(program_id: Pubkey) {
    let url = "ws://127.0.0.1:8900";

    let (_client, receiver) =
        PubsubClient::logs_subscribe(
            url,
            solana_client::rpc_config::RpcTransactionLogsFilter::Mentions(vec![
                program_id.to_string(),
            ]),
            solana_client::rpc_config::RpcTransactionLogsConfig { commitment: None },
        )
        .unwrap();

    println!("Listening Solana events...");

    for msg in receiver {
        for log in &msg.value.logs {
            if let Some(base64_data) = log.strip_prefix("Program data: ") {
                match general_purpose::STANDARD.decode(base64_data) {
                    Ok(bytes) => {
                        if bytes.len() < 8 {
                            eprintln!("Invalid event bytes: too short");
                            continue;
                        }

                        // 去掉 8 byte discriminator
                        let struct_bytes = &bytes[8..];

                        match OrderCreated::try_from_slice(struct_bytes) {
                            Ok(event) => println!("🔥 OrderCreated event: {:?}", event),
                            Err(e) => eprintln!("Failed to deserialize OrderCreated: {:?}", e),
                        }
                    }
                    Err(e) => eprintln!("Failed to decode base64: {:?}", e),
                }
            } else {
                println!("LOG: {}", log);
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let program_id = "YourProgramIdHere".parse().unwrap();
    std::thread::spawn(move || {
        listen(program_id);
    });

    loop {}
}
