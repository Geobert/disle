mod alias;
mod discord;

#[tokio::main]
async fn main() {
    discord::run().await;
}
