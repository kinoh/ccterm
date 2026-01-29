use serde::Serialize;

#[derive(Debug, Clone)]
pub enum Role {
    User,
    Assistant,
}

#[derive(Debug, Clone)]
pub struct TranscriptMessage {
    pub role: Role,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct IncomingMessage {
    pub text: String,
    pub conversation_id: String,
    pub thread_id: Option<String>,
    pub timestamp: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OutgoingMessage {
    pub text: String,
    pub conversation_id: String,
    pub thread_id: Option<String>,
}
