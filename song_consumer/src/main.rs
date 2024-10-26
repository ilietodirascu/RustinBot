use dotenvy::dotenv;
use futures_util::{future::join_all, StreamExt};
use lapin::{
    options::{BasicAckOptions, BasicConsumeOptions, BasicPublishOptions},
    types::FieldTable,
    BasicProperties, Channel, Connection, ConnectionProperties, Consumer,
};
use models::{ConvertResponse, RabbitMessage, Tomp3Response, YouTubeResponse};
use reqwest::{
    cookie::{CookieStore, Jar},
    Client,
};
use std::{env, error::Error, sync::Arc};
use urlencoding::encode;

mod models;

type DynError = Box<dyn Error + Send + Sync + 'static>;

#[tokio::main]
async fn main() -> Result<(), DynError> {
    pretty_env_logger::init();
    dotenv().expect("Failed to load .env file");
    log::info!("Application started");

    let rabbit_addr = env::var("RABBIT_ADDRESS")?;
    let google_api_key = env::var("GOOGLE_VISION_API_KEY")?;

    let connection = Connection::connect(&rabbit_addr, ConnectionProperties::default()).await?;
    log::info!("Connected to RabbitMQ at {}", rabbit_addr);

    let channel = connection.create_channel().await?;
    let mut consumer: Consumer = channel
        .basic_consume(
            "Music",
            "song_consumer",
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await?;
    log::info!("Waiting for messages on 'Music' queue...");

    while let Some(delivery) = consumer.next().await {
        match delivery {
            Ok(delivery) => {
                log::info!("Received message: {:?}", delivery);
                let message: RabbitMessage = serde_json::from_slice(&delivery.data)?;
                log::info!("Parsed message: {:?}", message);

                match process_songs(message.text, &google_api_key).await {
                    Ok(links) => {
                        publish_to_reply_queue(&channel, message.chat_id, links).await?;
                        delivery.ack(BasicAckOptions::default()).await?;
                        log::info!("Message processed and acknowledged successfully");
                    }
                    Err(e) => {
                        log::error!("Error processing message: {}", e);
                    }
                }
            }
            Err(e) => {
                log::error!("Failed to receive message: {}", e);
            }
        }
    }

    Ok(())
}

async fn process_songs(text: String, google_api_key: &str) -> Result<Vec<String>, DynError> {
    let cookie_jar = Arc::new(Jar::default());
    let mp3_client = Client::builder()
        .cookie_provider(cookie_jar) // Attach the cookie jar only for mp3 API requests
        .build()?;
    let general_client = Client::new(); // General client for other requests

    let songs: Vec<&str> = text.lines().collect();
    let mut tasks = Vec::new();

    for song in songs {
        let mp3_client = mp3_client.clone();
        let general_client = general_client.clone();
        let api_key = google_api_key.to_string();
        let song = song.to_string();

        let task = tokio::spawn(async move {
            log::info!("Processing song: {}", song);

            let video_id = search_youtube(&general_client, &api_key, &song)
                .await?
                .ok_or_else(|| Box::<dyn Error + Send + Sync>::from("No video found"))?;

            log::info!("Using video ID: {}", video_id);

            let k = get_tomp3_k(&mp3_client, &video_id)
                .await?
                .ok_or_else(|| Box::<dyn Error + Send + Sync>::from("Failed to get k parameter"))?;

            log::info!("Retrieved k parameter for video ID: {}", video_id);

            let dlink = convert_to_mp3(&mp3_client, &video_id, &k)
                .await?
                .ok_or_else(|| {
                    Box::<dyn Error + Send + Sync>::from("Failed to get download link")
                })?;

            log::info!("Retrieved download link: {}", dlink);

            // Return the formatted link with song name
            Ok::<String, DynError>(format!("🎵 *{}*\n🔗 {}", song, dlink))
        });

        tasks.push(task);
    }

    let results = join_all(tasks).await;
    let mut links = Vec::new();

    for (index, result) in results.into_iter().enumerate() {
        match result {
            Ok(Ok(link)) => links.push(format!("{}. {}", index + 1, link)),
            Ok(Err(e)) => log::error!("Error in task: {}", e),
            Err(e) => log::error!("Task panicked: {}", e),
        }
    }

    Ok(links)
}

async fn search_youtube(
    client: &Client,
    api_key: &str,
    query: &str,
) -> Result<Option<String>, DynError> {
    let encoded_query = encode(query);
    let url = format!(
        "https://www.googleapis.com/youtube/v3/search?part=snippet&type=video&order=viewCount&maxResults=1&q={}&key={}",
        encoded_query, api_key
    );

    log::info!("Searching YouTube with query: {}", query);
    let response: YouTubeResponse = client.get(&url).send().await?.json().await?;
    Ok(response
        .items
        .into_iter()
        .next()
        .map(|item| item.id.videoId))
}

fn log_cookies(cookie_jar: &Arc<Jar>, url: &str) {
    let cookies = cookie_jar.cookies(&url.parse().unwrap());
    match cookies {
        Some(cookie) => log::info!("Attached cookies for {}: {:#?}", url, cookie),
        None => log::info!("No cookies attached for {}", url),
    }
}

async fn get_tomp3_k(client: &Client, video_id: &str) -> Result<Option<String>, DynError> {
    let url = "https://tomp3.cc/api/ajax/search";
    let params = [
        (
            "query",
            format!("https://www.youtube.com/watch?v={}", video_id),
        ),
        ("vt", "downloader".to_string()),
    ];

    log::info!("Retrieving k parameter for video ID: {}", video_id);

    let response = client.post(url).form(&params).header("Cookie", "cf_clearance=nfBjEpAsDIH9gI2YRAWoVSkMrAyeiF2ArPYV9WMQop4-1723801695-1.0.1.1-C8QFuaiYCUF9A6Rz8LXox1TOt.xvGErsl_Is71Wyof3mkIu3RbEHxiIOO5z8icN05BoEAaPvkntWZRxWVAXFEw; _ga_JRWV2N11YN=GS1.1.1723801702.1.1.1723801732.0.0.0; _ga=GA1.1.1396507687.1723801703").send().await?;

    let status = response.status();
    let text = response.text().await?;
    log::info!("Response status: {}", status);
    log::info!("Raw response body: {}", text);

    if !status.is_success() {
        log::error!("Failed request: {}", status);
        return Err(Box::<dyn Error + Send + Sync>::from(
            "Non-successful status",
        ));
    }

    let parsed: Result<Tomp3Response, _> = serde_json::from_str(&text);
    match parsed {
        Ok(response) => Ok(response
            .links
            .and_then(|l| l.mp3)
            .and_then(|mp3| mp3.get("mp3128").map(|link| link.k.clone()))),
        Err(e) => {
            log::error!("Error decoding response: {}", e);
            Err(Box::<dyn Error + Send + Sync>::from(
                "Error decoding response body",
            ))
        }
    }
}

async fn convert_to_mp3(
    client: &Client,
    video_id: &str,
    k: &str,
) -> Result<Option<String>, DynError> {
    let url = "https://tomp3.cc/api/ajax/convert";
    let params = [("vid", video_id.to_string()), ("k", k.to_string())];

    log::info!("Converting video ID {} to MP3", video_id);
    let response: ConvertResponse = client.post(url).form(&params).send().await?.json().await?;
    Ok(Some(response.dlink))
}

async fn publish_to_reply_queue(
    channel: &Channel,
    chat_id: i64,
    links: Vec<String>,
) -> Result<(), DynError> {
    let message = RabbitMessage {
        chat_id,
        text: links.join("\n"),
    };
    let serialized_message = serde_json::to_vec(&message)?;
    channel
        .basic_publish(
            "",
            "Reply",
            BasicPublishOptions::default(),
            &serialized_message,
            BasicProperties::default(),
        )
        .await?;
    log::info!("Published reply for chat ID: {}", chat_id);
    Ok(())
}
