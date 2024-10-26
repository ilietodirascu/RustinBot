use dotenvy::dotenv;
use futures_util::StreamExt;
use lapin::{
    options::{BasicAckOptions, BasicConsumeOptions, BasicPublishOptions},
    types::FieldTable,
    BasicProperties, Channel, Connection, ConnectionProperties, Consumer,
};
use log::info;
use models::{
    Feature, ImageContent, RabbitMessage, VisionRequest, VisionRequestItem, VisionResponse,
};
use reqwest::Client;
use std::{env, error::Error};
mod models;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    pretty_env_logger::init();
    dotenv().expect("Failed to load .env file");

    let rabbit_addr = env::var("RABBIT_ADDRESS").expect("RABBIT_ADDRESS must be set");
    let telegram_token = env::var("TELEGRAM_BOT_TOKEN").expect("TELEGRAM_BOT_TOKEN must be set");
    let google_api_key =
        env::var("GOOGLE_VISION_API_KEY").expect("GOOGLE_VISION_API_KEY must be set");

    let connection = Connection::connect(&rabbit_addr, ConnectionProperties::default())
        .await
        .expect("Failed to connect to RabbitMQ");

    let channel = connection.create_channel().await?;
    let mut consumer: Consumer = channel
        .basic_consume(
            "ImageToText",    // Queue name
            "image_consumer", // Consumer tag
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await?;

    println!("Waiting for messages...");

    while let Some(delivery) = consumer.next().await {
        if let Ok(delivery) = delivery {
            let message: RabbitMessage =
                serde_json::from_slice(&delivery.data).expect("Failed to parse RabbitMessage");

            let base64_image = download_image_as_base64(&telegram_token, &message.text).await?;
            let extracted_text = detect_text_from_image(&google_api_key, &base64_image).await?;

            // Publish the reply message
            let reply_message = RabbitMessage {
                chat_id: message.chat_id,
                text: extracted_text,
            };
            publish_to_reply_queue(&channel, &reply_message).await?;

            // Acknowledge the message
            delivery.ack(BasicAckOptions::default()).await?;
        }
    }

    Ok(())
}

// Download image from Telegram and convert it to Base64
async fn download_image_as_base64(
    telegram_token: &str,
    file_id: &str,
) -> Result<String, Box<dyn Error>> {
    let client = Client::new();
    let file_path_url = format!(
        "https://api.telegram.org/bot{}/getFile?file_id={}",
        telegram_token, file_id
    );

    let file_path_response: serde_json::Value =
        client.get(&file_path_url).send().await?.json().await?;
    let file_path = file_path_response["result"]["file_path"]
        .as_str()
        .ok_or("File path not found")?;

    let download_url = format!(
        "https://api.telegram.org/file/bot{}/{}",
        telegram_token, file_path
    );
    let image_bytes = client.get(&download_url).send().await?.bytes().await?;
    let base64_image = base64::encode(&image_bytes);

    Ok(base64_image)
}

// Detect text from image using Google Vision API
async fn detect_text_from_image(
    google_api_key: &str,
    base64_image: &str,
) -> Result<String, Box<dyn Error>> {
    let client = Client::new();
    let request_body = VisionRequest {
        requests: vec![VisionRequestItem {
            image: ImageContent {
                content: base64_image.to_string(),
            },
            features: vec![Feature {
                r#type: "TEXT_DETECTION".to_string(),
            }],
        }],
    };

    let url = format!(
        "https://vision.googleapis.com/v1/images:annotate?key={}",
        google_api_key
    );

    let response: VisionResponse = client
        .post(&url)
        .json(&request_body)
        .send()
        .await?
        .json()
        .await?;

    let description = response
        .responses
        .first()
        .and_then(|r| r.textAnnotations.as_ref())
        .and_then(|annotations| annotations.first())
        .map(|annotation| annotation.description.clone())
        .unwrap_or_else(|| "No text found.".to_string());

    Ok(description)
}

// Publish message to the Reply queue
async fn publish_to_reply_queue(
    channel: &Channel,
    message: &RabbitMessage,
) -> Result<(), Box<dyn Error>> {
    let queue_name = "Reply";
    let serialized_message = serde_json::to_vec(&message).expect("Failed to serialize message");

    channel
        .basic_publish(
            "", // Exchange
            queue_name,
            BasicPublishOptions::default(),
            &serialized_message,
            BasicProperties::default(),
        )
        .await?;

    info!("Published message to Reply queue: {:?}", message);
    Ok(())
}
