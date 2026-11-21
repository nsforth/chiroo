use std::collections::{HashMap, HashSet};

use async_openai::types::chat::{ChatCompletionTool, ChatCompletionTools, FunctionObject};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use crate::domain::models::document::DocumentId;
use crate::domain::models::document_chunk::SearchedChunk;
use crate::domain::models::knowledge_base::{KnowledgeBase, KnowledgeBaseId};
use crate::domain::models::search_filter::SearchFilter;
use crate::domain::models::search_params::SearchParams;
use crate::domain::models::source::Source;
use crate::domain::models::tool::{LLMTool, ToolExecution};
use crate::services::knowledge_service::KnowledgeService;
use crate::services::search_service::SearchService;

// ─── SearchAllKbsTool ───

pub struct SearchAllKbsTool {
    search_service: SearchService,
    knowledge_service: KnowledgeService,
    params: SearchParams,
}

impl SearchAllKbsTool {
    pub fn new(
        search_service: SearchService,
        knowledge_service: KnowledgeService,
        params: SearchParams,
    ) -> Self {
        Self {
            search_service,
            knowledge_service,
            params,
        }
    }
}

#[async_trait]
impl LLMTool for SearchAllKbsTool {
    fn definition(&self) -> ChatCompletionTools {
        ChatCompletionTools::Function(ChatCompletionTool {
            function: FunctionObject {
                 name: "search_in_all_kbs".into(),
                 description: Some(
                     "Search across all knowledge bases (or a subset via `kb_ids`) for relevant document chunks. \
                      Use this as the first search when you do not know which knowledge base holds the answer. \
                      For targeted filtering by specific metadata fields, use `search_in_kb` with a particular \
                      base's id. Output: matching chunks with `document_id`, `kb_id`, `kb_name`, `title`, `url`, \
                      `content`, chunk position and `score`."
                         .into(),
                 ),
                 parameters: Some(serde_json::json!({
                     "type": "object",
                     "properties": {
                          "queries": {
                              "type": "array",
                              "items": { "type": "string" },
                              "description": "Array of natural language search queries (several distinct angles). \
                      Must be a JSON array of strings, e.g. [\"q1\", \"q2\"] — never a single string."
                          },
                          "kb_ids": {
                             "type": "array",
                             "items": { "type": "string" },
                             "description": "Knowledge base ids to search. Omit to search all knowledge bases."
                         }
                     },
                     "required": ["queries"]
                 })),
                strict: None,
            },
        })
    }

    async fn execute(&mut self, arguments: &str) -> Result<ToolExecution, String> {
        let args = parse_search_all_args(arguments)?;

        let kbs = self
            .knowledge_service
            .list_kbs()
            .await
            .map_err(|e| e.to_string())?;

        let kbs = filter_kbs(kbs, args.kb_ids)?;

        let kb_names: HashMap<Uuid, String> =
            kbs.iter().map(|kb| (kb.id.0, kb.name.clone())).collect();

        let chunks = self
            .search_service
            .search_all(args.queries, &kbs, &SearchFilter::default(), &self.params)
            .await
            .map_err(|e| e.to_string())?;

        let sources: Vec<Source> = chunks.iter().map(Source::from).collect();
        Ok(ToolExecution {
            output: serde_json::json!({ "chunks": search_chunks_to_json(&chunks, &kb_names) }),
            sources,
        })
    }
}

// ─── SearchInKbTool ───

pub struct SearchInKbTool {
    search_service: SearchService,
    knowledge_service: KnowledgeService,
    params: SearchParams,
}

impl SearchInKbTool {
    pub fn new(
        search_service: SearchService,
        knowledge_service: KnowledgeService,
        params: SearchParams,
    ) -> Self {
        Self {
            search_service,
            knowledge_service,
            params,
        }
    }
}

#[async_trait]
impl LLMTool for SearchInKbTool {
    fn definition(&self) -> ChatCompletionTools {
        ChatCompletionTools::Function(ChatCompletionTool {
            function: FunctionObject {
                name: "search_in_kb".into(),
                description: Some(
                    "Search inside ONE specific knowledge base for relevant document chunks. \
                     Use this for focused searches when you know which knowledge base to query — \
                     e.g. to apply metadata `filters` that are declared by that base (see the knowledge \
                     base descriptions for the list of its filterable fields with their types). Filters are \
                     validated against that base: an unknown field or a value of the wrong type returns an \
                     error explaining the problem."
                        .into(),
                ),
                parameters: Some(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "queries": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Array of natural language search queries (several distinct angles). \
                    Must be a JSON array of strings, e.g. [\"q1\", \"q2\"] — never a single string."
                        },
                        "kb_id": {
                            "type": "string",
                            "description": "The id of the knowledge base to search."
                        },
                        "filters": {
                            "type": "object",
                            "description": "Optional metadata filters, combined with AND. Keys must be fields declared by this knowledge base (see its description); a value must match the declared field type (keyword/uuid: string; bool: boolean; integer: integer; float: number; datetime: RFC 3339 string; ranges gt/gte/lt/lte only for integer, float, datetime). A bare value means equality: {\"reporter\": \"alice\"}. A range uses an object with any of the operators eq, gt, gte, lt, lte: {\"updated_at\": {\"gte\": \"2026-01-01T00:00:00Z\", \"lte\": \"2026-12-31T23:59:59Z\"}}. Datetime values are RFC 3339 strings."
                        }
                    },
                    "required": ["queries", "kb_id"]
                })),
                strict: None,
            },
        })
    }

    async fn execute(&mut self, arguments: &str) -> Result<ToolExecution, String> {
        let args = parse_search_in_kb_args(arguments)?;

        let kb = self
            .knowledge_service
            .get_kb(KnowledgeBaseId(args.kb_id))
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("knowledge base not found: {}", args.kb_id))?;

        let mut filters = args.filters;
        filters.validate(&kb.indexes).map_err(|e| e.to_string())?;

        let kb_names: HashMap<Uuid, String> = [(kb.id.0, kb.name.clone())].into_iter().collect();

        let chunks = self
            .search_service
            .search_in_kb(args.queries, &kb, &filters, &self.params)
            .await
            .map_err(|e| e.to_string())?;

        let sources: Vec<Source> = chunks.iter().map(Source::from).collect();
        Ok(ToolExecution {
            output: serde_json::json!({ "chunks": search_chunks_to_json(&chunks, &kb_names) }),
            sources,
        })
    }
}

#[derive(Debug)]
struct SearchAllArgs {
    queries: Vec<String>,
    kb_ids: Option<HashSet<Uuid>>,
}

#[derive(Debug)]
struct SearchInKbArgs {
    queries: Vec<String>,
    kb_id: Uuid,
    filters: SearchFilter,
}

#[derive(Debug, Deserialize)]
struct QueryItem(String);

#[derive(Debug, Deserialize)]
struct KbId(Uuid);

fn parse_queries(input: &Value) -> Result<Vec<String>, String> {
    let arr = input["queries"]
        .as_array()
        .ok_or_else(|| "Missing 'queries' array".to_string())?;
    let items: Vec<QueryItem> = serde_json::from_value(Value::Array(arr.clone()))
        .map_err(|e| format!("Invalid queries: {e}"))?;
    if items.is_empty() {
        return Err("At least one query is required".into());
    }
    Ok(items.into_iter().map(|QueryItem(query)| query).collect())
}

fn parse_filters(input: &Value) -> Result<SearchFilter, String> {
    match input.get("filters") {
        None | Some(Value::Null) => Ok(SearchFilter::default()),
        Some(v) => SearchFilter::from_llm_value(v).map_err(|e| e.to_string()),
    }
}

fn parse_search_all_args(arguments: &str) -> Result<SearchAllArgs, String> {
    let input: Value = serde_json::from_str(arguments).map_err(|e| format!("Invalid args: {e}"))?;
    let queries = parse_queries(&input)?;

    let kb_ids = match input["kb_ids"].as_array() {
        None => None,
        Some(arr) if arr.is_empty() => None,
        Some(arr) => Some(
            serde_json::from_value::<Vec<KbId>>(Value::Array(arr.clone()))
                .map_err(|e| e.to_string())?
                .into_iter()
                .map(|KbId(id)| id)
                .collect(),
        ),
    };

    Ok(SearchAllArgs { queries, kb_ids })
}

fn parse_search_in_kb_args(arguments: &str) -> Result<SearchInKbArgs, String> {
    let input: Value = serde_json::from_str(arguments).map_err(|e| format!("Invalid args: {e}"))?;
    let queries = parse_queries(&input)?;

    let kb_id = input["kb_id"].as_str().ok_or("Missing 'kb_id'")?;
    let kb_id =
        Uuid::parse_str(kb_id).map_err(|_| format!("Invalid knowledge base id: {kb_id}"))?;

    let filters = parse_filters(&input)?;

    Ok(SearchInKbArgs {
        queries,
        kb_id,
        filters,
    })
}

fn filter_kbs(
    kbs: Vec<KnowledgeBase>,
    kb_ids: Option<HashSet<Uuid>>,
) -> Result<Vec<KnowledgeBase>, String> {
    let Some(ids) = kb_ids else {
        return Ok(kbs);
    };
    let unknown: Vec<Uuid> = ids
        .difference(&kbs.iter().map(|kb| kb.id.0).collect())
        .copied()
        .collect();
    if !unknown.is_empty() {
        return Err(format!("Unknown knowledge base id: {}", unknown[0]));
    }
    Ok(kbs
        .into_iter()
        .filter(|kb| ids.contains(&kb.id.0))
        .collect())
}

fn search_chunks_to_json(chunks: &[SearchedChunk], kb_names: &HashMap<Uuid, String>) -> Value {
    let items: Vec<Value> = chunks
        .iter()
        .map(|chunk| {
            let kb_name = kb_names.get(&chunk.kb_id.0).cloned().unwrap_or_default();
            serde_json::json!({
                "document_id": chunk.document_id.0,
                "kb_id": chunk.kb_id.0,
                "kb_name": kb_name,
                "title": chunk.title,
                "url": chunk.url,
                "content": chunk.content,
                "chunk_offset": chunk.chunk_offset,
                "chunk_length": chunk.chunk_length,
                "document_total_length": chunk.document_total_length,
                "metadata": chunk.metadata,
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
                    "Read a range of a document discovered via search_in_all_kbs or search_in_kb. \
                     Pass document_id (from search results), offset (character offset within the \
                     document) and size (number of characters). To read the entire document, use \
                     offset=0 and size=document_total_length. Only works for documents returned \
                     by search_in_all_kbs or search_in_kb."
                        .into(),
                ),
                parameters: Some(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "document_id": {
                            "type": "string",
                            "description": "ID of the document to read, taken from search_in_all_kbs or search_in_kb results"
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

        let raw = String::from_utf8(doc_data.data)
            .map_err(|e| format!("Document content is not valid UTF-8: {e}"))?;

        let text = crate::utils::with_document_title(&doc.title, &raw);
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

#[derive(Debug, Deserialize)]
struct ExtractInput {
    #[serde(default)]
    offset: u64,
    #[serde(default)]
    size: Option<u64>,
}

fn parse_extract_args(arguments: &str) -> Result<ExtractArgs, String> {
    let input: Value = serde_json::from_str(arguments).map_err(|e| format!("Invalid args: {e}"))?;

    let document_id = input["document_id"]
        .as_str()
        .ok_or_else(|| "Missing 'document_id'".to_string())?
        .to_string();

    let ExtractInput { offset, size } =
        serde_json::from_value(input).map_err(|e| format!("Invalid args: {e}"))?;

    Ok(ExtractArgs {
        document_id,
        offset: offset as usize,
        size: size.map(|v| v as usize),
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
    use crate::domain::models::kb_index::KbIndexes;
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
        let kb_id = chunk.kb_id.0;
        let kb_names: HashMap<Uuid, String> =
            [(kb_id, "Company Docs".to_string())].into_iter().collect();
        let json = search_chunks_to_json(&[chunk], &kb_names);

        let first = &json[0];
        assert_eq!(first["document_id"], "doc-1");
        assert_eq!(first["kb_id"], kb_id.to_string());
        assert_eq!(first["kb_name"], "Company Docs");
        assert_eq!(first["title"], "Title");
        assert_eq!(first["url"], "https://example.com");
        assert_eq!(first["content"], "hello");
        assert_eq!(first["chunk_offset"], 10);
        assert_eq!(first["chunk_length"], 5);
        assert_eq!(first["document_total_length"], 100);
        assert_eq!(first["score"].as_f64().unwrap() as f32, 0.9f32);
    }

    #[test]
    fn search_chunks_to_json_omits_kb_name_when_unknown() {
        let chunk = make_chunk("doc-1", 10, 5, "hello", 0.9);
        let kb_id = chunk.kb_id.0;
        let json = search_chunks_to_json(&[chunk], &HashMap::new());

        let first = &json[0];
        assert_eq!(first["kb_id"], kb_id.to_string());
        assert_eq!(first["kb_name"], "");
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

    fn make_kb(id: Uuid, name: &str) -> KnowledgeBase {
        KnowledgeBase {
            id: KnowledgeBaseId(id),
            name: name.to_string(),
            description: String::new(),
            public: false,
            metadata: serde_json::json!({}),
            language: None,
            indexes: KbIndexes::default(),
            created_at: chrono::NaiveDateTime::MIN,
            updated_at: chrono::NaiveDateTime::MIN,
        }
    }

    #[test]
    fn parse_search_all_args_rejects_missing_queries() {
        assert!(parse_search_all_args("{}").is_err());
    }

    #[test]
    fn parse_search_all_args_rejects_empty_queries() {
        assert!(parse_search_all_args(r#"{"queries": []}"#).is_err());
    }

    #[test]
    fn parse_queries_rejects_missing() {
        assert!(parse_queries(&serde_json::json!({})).is_err());
    }

    #[test]
    fn parse_queries_rejects_empty() {
        assert!(parse_queries(&serde_json::json!({ "queries": [] })).is_err());
    }

    #[test]
    fn parse_queries_rejects_non_string_item() {
        let err = parse_queries(&serde_json::json!({ "queries": ["rust", 42] }))
            .expect_err("non-string query item must be rejected");
        assert!(
            err.contains("42"),
            "error must name the offending value: {err}"
        );
    }

    #[test]
    fn parse_search_all_args_accepts_queries_without_kb_ids() {
        let args = parse_search_all_args(r#"{"queries": ["a", "b"]}"#).unwrap();
        assert_eq!(args.queries, vec!["a".to_string(), "b".to_string()]);
        assert_eq!(args.kb_ids, None);
    }

    #[test]
    fn parse_search_all_args_parses_kb_ids() {
        let id = Uuid::new_v4();
        let args = parse_search_all_args(&format!(r#"{{"queries": ["a"], "kb_ids": ["{}"]}}"#, id))
            .unwrap();
        assert_eq!(args.kb_ids, Some(HashSet::from([id])));
    }

    #[test]
    fn parse_search_all_args_treats_empty_kb_ids_as_none() {
        let args = parse_search_all_args(r#"{"queries": ["a"], "kb_ids": []}"#).unwrap();
        assert_eq!(args.kb_ids, None);
    }

    #[test]
    fn parse_search_all_args_rejects_invalid_kb_id() {
        assert!(parse_search_all_args(r#"{"queries": ["a"], "kb_ids": ["not-a-uuid"]}"#).is_err());
    }

    #[test]
    fn parse_search_in_kb_args_requires_kb_id() {
        assert!(parse_search_in_kb_args(r#"{"queries": ["a"]}"#).is_err());
    }

    #[test]
    fn parse_search_in_kb_args_rejects_invalid_kb_id() {
        assert!(parse_search_in_kb_args(r#"{"queries": ["a"], "kb_id": "not-a-uuid"}"#).is_err());
    }

    #[test]
    fn parse_search_in_kb_args_parses_valid() {
        let id = Uuid::new_v4();
        let args = parse_search_in_kb_args(&format!(r#"{{"queries": ["a"], "kb_id": "{}"}}"#, id))
            .unwrap();
        assert_eq!(args.queries, vec!["a".to_string()]);
        assert_eq!(args.kb_id, id);
        assert!(args.filters.is_empty());
    }

    #[test]
    fn parse_search_in_kb_args_parses_valid_filters() {
        let id = Uuid::new_v4();
        let args = parse_search_in_kb_args(&format!(
            r#"{{"queries": ["a"], "kb_id": "{}", "filters": {{"topic": "rust"}}}}"#,
            id
        ))
        .unwrap();
        assert_eq!(args.filters.conditions().len(), 1);
    }

    #[test]
    fn parse_search_in_kb_args_rejects_unknown_filter_operator() {
        let id = Uuid::new_v4();
        let err = parse_search_in_kb_args(&format!(
            r#"{{"queries": ["a"], "kb_id": "{}", "filters": {{"x": {{"neq": 1}}}}}}"#,
            id
        ))
        .expect_err("unknown operator must be rejected");
        assert!(err.contains("\"x\""), "error must name the field: {err}");
    }

    #[test]
    fn filter_kbs_without_ids_returns_all() {
        let kbs = vec![make_kb(Uuid::new_v4(), "a"), make_kb(Uuid::new_v4(), "b")];
        let filtered = filter_kbs(kbs, None).unwrap();
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn filter_kbs_selects_known_ids() {
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let kbs = vec![make_kb(id1, "a"), make_kb(id2, "b")];
        let filtered = filter_kbs(kbs, Some(HashSet::from([id2]))).unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id.0, id2);
    }

    #[test]
    fn filter_kbs_rejects_unknown_id() {
        let kbs = vec![make_kb(Uuid::new_v4(), "a")];
        let result = filter_kbs(kbs, Some(HashSet::from([Uuid::new_v4()])));
        assert!(result.is_err());
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
