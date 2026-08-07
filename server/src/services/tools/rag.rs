use async_openai::types::chat::{ChatCompletionTool, ChatCompletionTools, FunctionObject};
use async_trait::async_trait;
use serde_json::Value;

use crate::domain::models::tool::LLMTool;

// ─── Mock data ───

fn mock_chunks() -> Value {
    serde_json::json!([
        {
            "chunk_id": "c1",
            "page_id": "p1",
            "score": 0.95,
            "snippet": "Rust is a systems programming language focused on safety, speed, and concurrency. It prevents segfaults and guarantees thread safety."
        },
        {
            "chunk_id": "c2",
            "page_id": "p1",
            "score": 0.87,
            "snippet": "Rust's ownership system enables memory safety without garbage collection. Each value has exactly one owner at a time."
        },
        {
            "chunk_id": "c3",
            "page_id": "p2",
            "score": 0.82,
            "snippet": "The Rust compiler enforces strict rules at compile time, catching many classes of bugs before the program runs."
        },
        {
            "chunk_id": "c4",
            "page_id": "p3",
            "score": 0.76,
            "snippet": "Traits in Rust define shared behavior. They are similar to interfaces in other languages but more powerful through associated types and blanket implementations."
        },
        {
            "chunk_id": "c5",
            "page_id": "p2",
            "score": 0.71,
            "snippet": "Pattern matching in Rust is exhaustive. The `match` expression forces handling of all possible cases at compile time."
        }
    ])
}

fn mock_pages(page_ids: &[String]) -> Value {
    let all_pages = serde_json::json!({
        "p1": {
            "page_id": "p1",
            "title": "The Rust Programming Language — Ownership",
            "full_text": "Rust is a systems programming language focused on safety, speed, and concurrency. It prevents segfaults and guarantees thread safety.\n\nRust's ownership system enables memory safety without garbage collection. Each value in Rust has exactly one owner at a time. When the owner goes out of scope, the value is dropped.\n\nBorrowing allows references to data without taking ownership. References are checked at compile time to prevent dangling pointers and data races."
        },
        "p2": {
            "page_id": "p2",
            "title": "The Rust Programming Language — Enums and Pattern Matching",
            "full_text": "Enums in Rust are algebraic data types that can represent one of several variants. Each variant can carry associated data.\n\nPattern matching in Rust is exhaustive. The `match` expression forces handling of all possible cases at compile time. The Rust compiler enforces strict rules at compile time, catching many classes of bugs before the program runs.\n\nThe `if let` syntax provides a concise way to handle a single pattern while ignoring others."
        },
        "p3": {
            "page_id": "p3",
            "title": "The Rust Programming Language — Traits and Generics",
            "full_text": "Traits in Rust define shared behavior. They are similar to interfaces in other languages but more powerful through associated types and blanket implementations.\n\nGenerics allow writing code that works with multiple types. Rust implements generics through monomorphization, producing specialized code for each concrete type used."
        }
    });

    let mut pages = Vec::new();
    for id in page_ids {
        if let Some(page) = all_pages.get(id) {
            pages.push(page.clone());
        }
    }
    serde_json::json!({ "pages": pages })
}

// ─── SearchRagTool ───

pub struct SearchRagTool;

#[async_trait]
impl LLMTool for SearchRagTool {
    fn definition(&self) -> ChatCompletionTools {
        ChatCompletionTools::Function(ChatCompletionTool {
            function: FunctionObject {
                name: "search_rag".into(),
                description: Some(
                    "Search the knowledge base for relevant document chunks. \
                     Takes an array of natural language queries and returns scored chunks with metadata."
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

    async fn execute(&mut self, arguments: &str) -> Result<Value, String> {
        let input: Value =
            serde_json::from_str(arguments).map_err(|e| format!("Invalid args: {e}"))?;

        let queries = input["queries"]
            .as_array()
            .ok_or("Missing 'queries' array")?;

        if queries.is_empty() {
            return Err("At least one query is required".into());
        }

        Ok(serde_json::json!({
            "chunks": mock_chunks()
        }))
    }
}

// ─── ExtractPageTool ───

pub struct ExtractPageTool;

#[async_trait]
impl LLMTool for ExtractPageTool {
    fn definition(&self) -> ChatCompletionTools {
        ChatCompletionTools::Function(ChatCompletionTool {
            function: FunctionObject {
                name: "extract_page".into(),
                description: Some(
                    "Retrieve the full text content of specific pages by their IDs. \
                     Takes an array of page IDs and returns the title and full text for each."
                        .into(),
                ),
                parameters: Some(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "page_ids": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Array of page IDs to retrieve"
                        }
                    },
                    "required": ["page_ids"]
                })),
                strict: None,
            },
        })
    }

    async fn execute(&mut self, arguments: &str) -> Result<Value, String> {
        let input: Value =
            serde_json::from_str(arguments).map_err(|e| format!("Invalid args: {e}"))?;

        let page_ids: Vec<String> = input["page_ids"]
            .as_array()
            .ok_or("Missing 'page_ids' array")?
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();

        if page_ids.is_empty() {
            return Err("At least one page_id is required".into());
        }

        Ok(mock_pages(&page_ids))
    }
}

// ─── Tests ───

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn search_rag_definition_has_correct_name() {
        let tool = SearchRagTool;
        let def = tool.definition();
        if let ChatCompletionTools::Function(ct) = def {
            assert_eq!(ct.function.name, "search_rag");
        } else {
            panic!("Expected Function variant");
        }
    }

    #[tokio::test]
    async fn search_rag_execute_returns_chunks() {
        let mut tool = SearchRagTool;
        let result = tool
            .execute(r#"{"queries": ["rust ownership", "traits"]}"#)
            .await
            .expect("Search should succeed");

        let chunks = result["chunks"]
            .as_array()
            .expect("Should have chunks array");
        assert!(!chunks.is_empty(), "Should return chunks");

        let first = &chunks[0];
        assert!(first["chunk_id"].is_string());
        assert!(first["page_id"].is_string());
        assert!(first["score"].is_number());
        assert!(first["snippet"].is_string());
    }

    #[tokio::test]
    async fn search_rag_rejects_empty_queries() {
        let mut tool = SearchRagTool;
        let result = tool.execute(r#"{"queries": []}"#).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn search_rag_rejects_missing_queries() {
        let mut tool = SearchRagTool;
        let result = tool.execute(r#"{}"#).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn extract_page_definition_has_correct_name() {
        let tool = ExtractPageTool;
        let def = tool.definition();
        if let ChatCompletionTools::Function(ct) = def {
            assert_eq!(ct.function.name, "extract_page");
        } else {
            panic!("Expected Function variant");
        }
    }

    #[tokio::test]
    async fn extract_page_execute_returns_pages() {
        let mut tool = ExtractPageTool;
        let result = tool
            .execute(r#"{"page_ids": ["p1", "p3"]}"#)
            .await
            .expect("Extract should succeed");

        let pages = result["pages"].as_array().expect("Should have pages array");
        assert!(!pages.is_empty(), "Should return pages");

        let first = &pages[0];
        assert!(first["page_id"].is_string());
        assert!(first["title"].is_string());
        assert!(first["full_text"].is_string());
    }

    #[tokio::test]
    async fn extract_page_skips_unknown_ids() {
        let mut tool = ExtractPageTool;
        let result = tool
            .execute(r#"{"page_ids": ["unknown", "p1"]}"#)
            .await
            .expect("Extract should succeed");

        let pages = result["pages"].as_array().expect("Should have pages array");
        assert_eq!(pages.len(), 1, "Should return only known pages");
    }

    #[tokio::test]
    async fn extract_page_rejects_empty_page_ids() {
        let mut tool = ExtractPageTool;
        let result = tool.execute(r#"{"page_ids": []}"#).await;
        assert!(result.is_err());
    }
}
