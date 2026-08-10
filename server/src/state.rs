use crate::config::Config;
use crate::infra::clients::embedding_client::EmbeddingClient;
use crate::infra::clients::qdrant_client::QdrantClient;
use crate::infra::clients::rerank_client::RerankClient;
use crate::services::ai_service::AiService;
use crate::services::auth_service::AuthService;
use crate::services::chat_service::ChatService;
use crate::services::knowledge_service::KnowledgeService;
use crate::services::message_service::MessageService;
use crate::services::vote_service::VoteService;

#[derive(Clone)]
pub struct AppState {
    pub config: &'static Config,
    pub chat_service: ChatService,
    pub message_service: MessageService,
    pub vote_service: VoteService,
    pub ai_service: AiService,
    pub auth_service: AuthService,
    pub knowledge_service: KnowledgeService,
    pub qdrant_client: QdrantClient,
    pub embedding_client: EmbeddingClient,
    pub rerank_client: RerankClient,
}
