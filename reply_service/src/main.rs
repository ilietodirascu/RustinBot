use dotenvy::dotenv;
use futures_util::StreamExt;
use lapin::{
    options::{BasicAckOptions, BasicConsumeOptions},
    types::FieldTable,
    Connection, ConnectionProperties, Consumer,
};
use serde::Deserialize;
use std::{env, error::Error};
use teloxide::{prelude::*, types::ChatId, Bot};

// Define the structure of the expected message
#[derive(Deserialize)]
struct RabbitMessage {
    chat_id: i64,
    text: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Initialize the logger and load the .env file
    pretty_env_logger::init();
    dotenv().expect("Failed to load .env file");

    // Retrieve RabbitMQ address and connect
    let rabbit_addr = env::var("RABBIT_ADDRESS").expect("RABBIT_ADDRESS must be set");
    let connection = Connection::connect(&rabbit_addr, ConnectionProperties::default())
        .await
        .expect("Failed to connect to RabbitMQ");

    let channel = connection.create_channel().await?;
    let mut consumer: Consumer = channel
        .basic_consume(
            "Reply",          // Queue name
            "reply_consumer", // Consumer tag
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await?;

    println!("Waiting for messages...");

    // Initialize the bot from environment variables
    let bot = Bot::from_env();

    // Process incoming messages from RabbitMQ
    while let Some(delivery) = consumer.next().await {
        if let Ok(delivery) = delivery {
            // Parse the message as JSON
            match serde_json::from_slice::<RabbitMessage>(&delivery.data) {
                Ok(rabbit_message) => {
                    println!(
                        "Received message for chat_id {}: {}",
                        rabbit_message.chat_id, rabbit_message.text
                    );

                    // Send a message to the specified chat_id
                    let chat_id = ChatId(rabbit_message.chat_id);
                    if let Err(err) = bot.send_message(chat_id, rabbit_message.text.clone()).await {
                        eprintln!("Failed to send message: {}", err);
                    }
                }
                Err(err) => {
                    eprintln!("Failed to parse message: {}", err);
                }
            }

            // Acknowledge the message
            delivery.ack(BasicAckOptions::default()).await?;
        }
    }

    Ok(())
}
