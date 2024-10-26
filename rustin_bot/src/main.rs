use log::info;
use teloxide::prelude::*;

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    log::info!("Starting throw dice bot...");

    let bot = Bot::from_env();

    teloxide::repl(bot, |bot: Bot, msg: Message| async move {
        match serde_json::to_string_pretty(&msg) {
            Ok(json) => {
                info!("Received message: {}", json);
            }
            Err(e) => {
                log::error!("Failed to convert message to JSON: {}", e);
            }
        }

        // Example: Send a dice emoji for fun
        bot.send_dice(msg.chat.id).await?;

        Ok(())
    })
    .await;
}
