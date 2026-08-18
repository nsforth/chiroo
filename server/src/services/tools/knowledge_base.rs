use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use async_openai::types::chat::{ChatCompletionTool, ChatCompletionTools, FunctionObject};
use async_trait::async_trait;
use serde_json::Value;

use crate::domain::models::document::DocumentId;
use crate::domain::models::document_chunk::SearchedChunk;
use crate::domain::models::source::Source;
use crate::domain::models::tool::{LLMTool, ToolExecution};
use crate::services::knowledge_service::KnowledgeService;
use crate::services::search_service::SearchService;

// ─── SearchKnowledgeBaseTool ───

pub struct SearchKnowledgeBaseTool {
    search_service: SearchService,
    knowledge_service: KnowledgeService,
}

impl SearchKnowledgeBaseTool {
    pub fn new(search_service: SearchService, knowledge_service: KnowledgeService) -> Self {
        Self {
            search_service,
            knowledge_service,
        }
    }
}

#[async_trait]
impl LLMTool for SearchKnowledgeBaseTool {
    fn definition(&self) -> ChatCompletionTools {
        ChatCompletionTools::Function(ChatCompletionTool {
            function: FunctionObject {
                name: "search_knowledge_base".into(),
                description: Some(
                    "Search the knowledge base for relevant document chunks. \
                     Takes an array of natural language queries and returns matching chunks with \
                     their character-based location within each document (chunk_offset, \
                     chunk_length, document_total_length). Use this first to discover documents \
                     and chunks; then call extract_document to read the full text (or a specific \
                     range) of the most promising documents."
                        .into(),
                ),
                parameters: Some(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "queries": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Array of natural language search queries"
                        }
                    },
                    "required": ["queries"]
                })),
                strict: None,
            },
        })
    }

    async fn execute(&mut self, arguments: &str) -> Result<ToolExecution, String> {
        let queries = parse_queries(arguments)?;

        let kbs = self
            .knowledge_service
            .list_kbs()
            .await
            .map_err(|e| e.to_string())?;

        let results = self
            .search_service
            .search(queries, &kbs)
            .await
            .map_err(|e| e.to_string())?;

        let chunks = flatten_search_results(results);
        let sources: Vec<Source> = chunks.iter().map(Source::from).collect();
        Ok(ToolExecution {
            output: serde_json::json!({ "chunks": search_chunks_to_json(&chunks) }),
            sources,
        })
    }
}

fn parse_queries(arguments: &str) -> Result<Vec<String>, String> {
    let input: Value = serde_json::from_str(arguments).map_err(|e| format!("Invalid args: {e}"))?;

    let queries: Vec<String> = input["queries"]
        .as_array()
        .ok_or("Missing 'queries' array")?
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect();

    if queries.is_empty() {
        return Err("At least one query is required".into());
    }

    Ok(queries)
}

fn flatten_search_results(results: HashMap<String, Vec<SearchedChunk>>) -> Vec<SearchedChunk> {
    let mut seen = HashSet::new();
    let mut chunks: Vec<SearchedChunk> = results
        .into_values()
        .flatten()
        .filter(|chunk| seen.insert(chunk.id.clone()))
        .collect();

    chunks.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(Ordering::Equal));

    chunks
}

fn search_chunks_to_json(chunks: &[SearchedChunk]) -> Value {
    let items: Vec<Value> = chunks
        .iter()
        .map(|chunk| {
            serde_json::json!({
                "document_id": chunk.document_id.0,
                "title": chunk.title,
                "url": chunk.url,
                "content": chunk.content,
                "chunk_offset": chunk.chunk_offset,
                "chunk_length": chunk.chunk_length,
                "document_total_length": chunk.document_total_length,
                "score": chunk.score,
            })
        })
        .collect();
    Value::Array(items)
}

// ─── ExtractDocumentTool ───

pub struct ExtractDocumentTool {
    knowledge_service: KnowledgeService,
}

impl ExtractDocumentTool {
    pub fn new(knowledge_service: KnowledgeService) -> Self {
        Self { knowledge_service }
    }
}

#[async_trait]
impl LLMTool for ExtractDocumentTool {
    fn definition(&self) -> ChatCompletionTools {
        ChatCompletionTools::Function(ChatCompletionTool {
            function: FunctionObject {
                name: "extract_document".into(),
                description: Some(
                    "Read a range of a document discovered via search_knowledge_base. \
                     Pass document_id (from search results), offset (character offset within the \
                     document) and size (number of characters). To read the entire document, use \
                     offset=0 and size=document_total_length. Only works for documents returned \
                     by search_knowledge_base."
                        .into(),
                ),
                parameters: Some(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "document_id": {
                            "type": "string",
                            "description": "ID of the document to read, taken from search_knowledge_base results"
                        },
                        "offset": {
                            "type": "integer",
                            "description": "Character offset within the document to start reading from (defaults to 0)"
                        },
                        "size": {
                            "type": "integer",
                            "description": "Number of characters to read (defaults to the rest of the document)"
                        }
                    },
                    "required": ["document_id"]
                })),
                strict: None,
            },
        })
    }

    async fn execute(&mut self, arguments: &str) -> Result<ToolExecution, String> {
        let args = parse_extract_args(arguments)?;

        let (doc, doc_data) = self
            .knowledge_service
            .get_document_with_data(&DocumentId(args.document_id.clone()))
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Document not found: {}", args.document_id))?;

        let text = String::from_utf8(doc_data.data)
            .map_err(|e| format!("Document content is not valid UTF-8: {e}"))?;

        let extracted = extract_document_slice(&text, args.offset, args.size)?;

        let source = Source {
            source_id: doc.id.0.clone(),
            url: doc.url.clone(),
            title: Some(doc.title.clone()),
            score: None,
        };

        Ok(ToolExecution {
            output: serde_json::json!({
                "document_id": doc.id.0,
                "title": doc.title,
                "url": doc.url,
                "content": extracted.content,
                "offset": extracted.offset,
                "size": extracted.size,
                "document_total_length": extracted.document_total_length,
            }),
            sources: vec![source],
        })
    }
}

struct ExtractArgs {
    document_id: String,
    offset: usize,
    size: Option<usize>,
}

fn parse_extract_args(arguments: &str) -> Result<ExtractArgs, String> {
    let input: Value = serde_json::from_str(arguments).map_err(|e| format!("Invalid args: {e}"))?;

    let document_id = input["document_id"]
        .as_str()
        .ok_or("Missing 'document_id'")?
        .to_string();

    let offset = input["offset"].as_u64().unwrap_or(0) as usize;
    let size = input["size"].as_u64().map(|v| v as usize);

    Ok(ExtractArgs {
        document_id,
        offset,
        size,
    })
}

struct ExtractedSlice {
    content: String,
    offset: usize,
    size: usize,
    document_total_length: usize,
}

fn extract_document_slice(
    text: &str,
    offset: usize,
    size: Option<usize>,
) -> Result<ExtractedSlice, String> {
    let char_starts: Vec<usize> = text.char_indices().map(|(byte, _)| byte).collect();
    let total = char_starts.len();

    let start = offset.min(total);
    let end = match size {
        Some(size) => start.saturating_add(size).min(total),
        None => total,
    };

    let start_byte = char_starts.get(start).copied().unwrap_or(text.len());
    let end_byte = char_starts.get(end).copied().unwrap_or(text.len());

    Ok(ExtractedSlice {
        content: text[start_byte..end_byte].to_string(),
        offset: start,
        size: end - start,
        document_total_length: total,
    })
}

// ─── Tests ───

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::models::document_chunk::DocumentChunkId;
    use crate::domain::models::knowledge_base::KnowledgeBaseId;
    use uuid::Uuid;

    fn make_chunk(
        document_id: &str,
        offset: usize,
        length: usize,
        content: &str,
        score: f32,
    ) -> SearchedChunk {
        SearchedChunk {
            id: DocumentChunkId(Uuid::new_v4()),
            document_id: DocumentId(document_id.to_string()),
            kb_id: KnowledgeBaseId(Uuid::new_v4()),
            chunk_index: 0,
            chunk_total: 1,
            chunk_offset: offset,
            chunk_length: length,
            document_total_length: 100,
            content: content.to_string(),
            version: "v1".to_string(),
            title: "Title".to_string(),
            url: "https://example.com".to_string(),
            metadata: serde_json::json!({}),
            score,
        }
    }

    #[test]
    fn search_chunks_to_json_maps_domain_fields() {
        let chunk = make_chunk("doc-1", 10, 5, "hello", 0.9);
        let json = search_chunks_to_json(&[chunk]);

        let first = &json[0];
        assert_eq!(first["document_id"], "doc-1");
        assert_eq!(first["title"], "Title");
        assert_eq!(first["url"], "https://example.com");
        assert_eq!(first["content"], "hello");
        assert_eq!(first["chunk_offset"], 10);
        assert_eq!(first["chunk_length"], 5);
        assert_eq!(first["document_total_length"], 100);
        assert_eq!(first["score"].as_f64().unwrap() as f32, 0.9f32);
    }

    #[test]
    fn source_from_searched_chunk_maps_domain_fields() {
        let chunk = make_chunk("doc-1", 10, 5, "hello", 0.9);
        let source = Source::from(&chunk);

        assert_eq!(source.source_id, "doc-1");
        assert_eq!(source.url, "https://example.com");
        assert_eq!(source.title.as_deref(), Some("Title"));
        assert_eq!(source.score, Some(0.9));
    }

    #[test]
    fn flatten_search_results_dedupes_and_sorts_by_score() {
        let a = make_chunk("doc-1", 0, 3, "low", 0.2);
        let b = make_chunk("doc-2", 0, 3, "high", 0.9);
        let mut results = HashMap::new();
        results.insert("q1".to_string(), vec![a.clone()]);
        results.insert("q2".to_string(), vec![a.clone(), b.clone()]);

        let flattened = flatten_search_results(results);
        assert_eq!(
            flattened.len(),
            2,
            "duplicates across queries must be removed"
        );
        assert!(flattened[0].score >= flattened[1].score);
    }

    #[test]
    fn parse_queries_rejects_missing() {
        assert!(parse_queries("{}").is_err());
    }

    #[test]
    fn parse_queries_rejects_empty() {
        assert!(parse_queries(r#"{"queries": []}"#).is_err());
    }

    #[test]
    fn parse_queries_accepts_strings() {
        let queries = parse_queries(r#"{"queries": ["a", "b"]}"#).unwrap();
        assert_eq!(queries, vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn parse_extract_args_defaults() {
        let args = parse_extract_args(r#"{"document_id": "doc-1"}"#).unwrap();
        assert_eq!(args.document_id, "doc-1");
        assert_eq!(args.offset, 0);
        assert_eq!(args.size, None);
    }

    #[test]
    fn parse_extract_args_with_range() {
        let args =
            parse_extract_args(r#"{"document_id": "doc-1", "offset": 5, "size": 10}"#).unwrap();
        assert_eq!(args.offset, 5);
        assert_eq!(args.size, Some(10));
    }

    #[test]
    fn extract_document_slice_reads_full_document_by_default() {
        let text = "hello world";
        let slice = extract_document_slice(text, 0, None).unwrap();
        assert_eq!(slice.content, "hello world");
        assert_eq!(slice.offset, 0);
        assert_eq!(slice.size, 11);
        assert_eq!(slice.document_total_length, 11);
    }

    #[test]
    fn extract_document_slice_reads_range() {
        let text = "hello world";
        let slice = extract_document_slice(text, 6, Some(5)).unwrap();
        assert_eq!(slice.content, "world");
        assert_eq!(slice.offset, 6);
        assert_eq!(slice.size, 5);
    }

    #[test]
    fn extract_document_slice_clamps_out_of_bounds() {
        let text = "hello";
        let slice = extract_document_slice(text, 3, Some(100)).unwrap();
        assert_eq!(slice.content, "lo");
        assert_eq!(slice.offset, 3);
        assert_eq!(slice.size, 2);
    }

    #[test]
    fn extract_document_slice_handles_unicode_by_char() {
        // Cyrillic text: 2 bytes per char.
        let text = "Привет мир";
        let slice = extract_document_slice(text, 7, Some(3)).unwrap();
        assert_eq!(slice.content, "мир");
        assert_eq!(slice.size, 3);
    }
}
