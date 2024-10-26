use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct RabbitMessage {
    pub chat_id: i64,
    pub text: String,
}

#[derive(Deserialize)]
pub struct YouTubeResponse {
    pub items: Vec<YouTubeItem>,
}

#[derive(Deserialize)]
pub struct YouTubeItem {
    pub id: YouTubeVideoId,
}

#[derive(Deserialize)]
pub struct YouTubeVideoId {
    pub videoId: String,
}

#[derive(Deserialize)]
pub struct Tomp3Response {
    pub links: Option<Links>,
}

#[derive(Deserialize)]
pub struct Links {
    pub mp3: Option<std::collections::HashMap<String, Mp3Link>>,
}

#[derive(Deserialize)]
pub struct Mp3Link {
    pub k: String,
}

#[derive(Deserialize)]
pub struct ConvertResponse {
    pub dlink: String,
}
