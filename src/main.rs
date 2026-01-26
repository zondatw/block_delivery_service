use solana_client::pubsub_client::PubsubClient;
use solana_sdk::pubkey::Pubkey;

pub fn listen(program_id: Pubkey) {
    let url = "ws://127.0.0.1:8900";

    let (_client, receiver) =
        PubsubClient::logs_subscribe(
            url,
            solana_client::rpc_config::RpcTransactionLogsFilter::Mentions(vec![
                program_id.to_string(),
            ]),
            solana_client::rpc_config::RpcTransactionLogsConfig {
                commitment: None,
            },
        )
        .unwrap();

    for msg in receiver {
        println!("LOG EVENT: {:?}", msg.value.logs);
        // TODO: parse OrderCreated
    }
}

#[tokio::main]
async fn main() {
    let program_id = "YourProgramIdHere".parse().unwrap();
    std::thread::spawn(move || {
        listen(program_id);
    });

    println!("Listening Solana events...");
    loop {}
}
