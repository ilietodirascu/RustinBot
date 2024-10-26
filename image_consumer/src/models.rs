use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug, Serialize)]
pub struct RabbitMessage {
    pub chat_id: i64,
    pub text: String, // This will store the Telegram file_id
}

#[derive(Serialize)]
pub struct VisionRequest {
    pub requests: Vec<VisionRequestItem>,
}

#[derive(Serialize)]
pub struct VisionRequestItem {
    pub image: ImageContent,
    pub features: Vec<Feature>,
}

#[derive(Serialize)]
pub struct ImageContent {
    pub content: String, // Base64-encoded image
}

#[derive(Serialize)]
pub struct Feature {
    pub r#type: String, // "TEXT_DETECTION"
}

#[derive(Deserialize, Debug)]
pub struct VisionResponse {
    pub responses: Vec<TextAnnotations>,
}

#[derive(Deserialize, Debug)]
pub struct TextAnnotations {
    pub textAnnotations: Option<Vec<Annotation>>,
}

#[derive(Deserialize, Debug)]
pub struct Annotation {
    pub description: String, // Extracted text from the image
}
