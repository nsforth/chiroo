use text_splitter::MarkdownSplitter;
use tracing::{info, instrument};

use crate::domain::models::document::Document;
use crate::domain::models::document_chunk::{DocumentChunk, DocumentChunkId};
use crate::domain::models::knowledge_base::KnowledgeBaseId;
use crate::utils::with_document_title;

#[derive(Clone)]
pub struct ChunkingService {
    max_chunk_size: usize,
}

impl ChunkingService {
    pub fn new(max_chunk_size: usize) -> Self {
        Self { max_chunk_size }
    }

    #[instrument(skip(self, text), fields(text_len = text.len()))]
    pub fn chunk_markdown(
        &self,
        kb_id: KnowledgeBaseId,
        document: &Document,
        text: &str,
    ) -> Vec<DocumentChunk> {
        if text.is_empty() {
            info!(status = "success");
            return Vec::new();
        }

        let text = with_document_title(&document.title, text);

        let splitter = MarkdownSplitter::new(self.max_chunk_size);
        let chunks_raw: Vec<(usize, &str)> = splitter.chunk_indices(&text).collect();
        let chunk_total = chunks_raw.len();
        let document_total_length = text.chars().count();

        // Offsets and lengths are character-based (Unicode scalar values), not
        // bytes, so that the whole pipeline (chunk → search → extract) stays
        // consistent across languages with multi-byte characters.
        let char_starts: Vec<usize> = text.char_indices().map(|(byte, _)| byte).collect();

        let result: Vec<DocumentChunk> = chunks_raw
            .into_iter()
            .enumerate()
            .map(|(chunk_index, (byte_offset, content))| DocumentChunk {
                id: DocumentChunkId::from_doc_id_and_chunk_idx(&document.id, chunk_index),
                document_id: document.id.clone(),
                kb_id,
                chunk_index,
                chunk_total,
                chunk_offset: char_starts.partition_point(|&byte| byte < byte_offset),
                chunk_length: content.chars().count(),
                document_total_length,
                content: content.to_string(),
                embedding: Vec::new(),
                version: document.version.clone(),
                title: document.title.clone(),
                url: document.url.clone(),
                metadata: document.metadata.clone(),
            })
            .collect();

        info!(chunk_total, status = "success");
        result
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDateTime;
    use uuid::Uuid;

    use super::*;
    use crate::domain::models::document::{DocumentId, MimeType};
    use crate::domain::models::knowledge_base::KnowledgeBaseId;

    fn make_kb_id() -> KnowledgeBaseId {
        KnowledgeBaseId(Uuid::new_v4())
    }

    fn make_document(id: &str, kb_id: KnowledgeBaseId, title: &str, url: &str) -> Document {
        Document {
            id: DocumentId(id.to_string()),
            kb_id,
            title: title.to_string(),
            url: url.to_string(),
            version: "v1".to_string(),
            mime_type: MimeType::TextPlain,
            metadata: serde_json::json!({"source": "test"}),
            created_at: NaiveDateTime::MIN,
            updated_at: NaiveDateTime::MIN,
        }
    }

    #[test]
    fn empty_text_returns_no_chunks() {
        let service = ChunkingService::new(512);
        let kb_id = make_kb_id();
        let doc = make_document("doc-1", kb_id, "Empty Doc", "https://example.com/empty");

        let chunks = service.chunk_markdown(kb_id, &doc, "");

        assert!(chunks.is_empty());
    }

    #[test]
    fn text_smaller_than_max_size_returns_single_chunk() {
        let service = ChunkingService::new(1000);
        let kb_id = make_kb_id();
        let doc = make_document("doc-1", kb_id, "Small Doc", "https://example.com/small");

        let text = "This is a short markdown paragraph.";
        let chunks = service.chunk_markdown(kb_id, &doc, text);

        assert_eq!(chunks.len(), 1);
        let chunk = &chunks[0];
        let expected = format!("# {}\n\n{text}", doc.title);
        assert_eq!(chunk.content, expected);
        assert_eq!(chunk.chunk_index, 0);
        assert_eq!(chunk.chunk_total, 1);
        assert_eq!(chunk.chunk_offset, 0);
        assert_eq!(chunk.chunk_length as usize, expected.chars().count());
        assert_eq!(
            chunk.document_total_length as usize,
            expected.chars().count()
        );
        assert_eq!(chunk.document_id, DocumentId("doc-1".to_string()));
        assert_eq!(chunk.kb_id, kb_id);
        assert_eq!(chunk.title, "Small Doc");
        assert_eq!(chunk.url, "https://example.com/small");
        assert_eq!(chunk.version, "v1");
        assert_eq!(chunk.metadata, serde_json::json!({"source": "test"}));
        assert!(chunk.embedding.is_empty());
    }

    #[test]
    fn multiple_chunks_with_correct_indices_and_offsets() {
        let service = ChunkingService::new(10);
        let kb_id = make_kb_id();
        let doc = make_document("doc-1", kb_id, "Multi", "https://example.com/multi");

        let text = "# Header\n\na longer paragraph that exceeds ten";
        let expected = format!("# {}\n\n{text}", doc.title);
        let chunks = service.chunk_markdown(kb_id, &doc, text);

        assert!(chunks.len() > 1, "should produce multiple chunks");
        assert_eq!(
            chunks.iter().map(|c| c.chunk_total).collect::<Vec<_>>(),
            vec![chunks.len(); chunks.len()],
            "all chunks must report the same chunk_total"
        );

        for (i, chunk) in chunks.iter().enumerate() {
            assert_eq!(
                chunk.chunk_index, i,
                "chunk {i} must have chunk_index = {i}"
            );
            assert!(
                !chunk.content.is_empty(),
                "chunk {i} must have non-empty content"
            );
            assert_eq!(
                chunk.document_total_length as usize,
                expected.chars().count(),
                "document_total_length must equal augmented text length"
            );
            assert_eq!(
                chunk.document_id,
                DocumentId("doc-1".to_string()),
                "document_id must match"
            );
        }

        // First chunk starts at offset 0 and carries the title header.
        assert_eq!(chunks[0].chunk_offset, 0);
        assert!(
            chunks[0].content.starts_with(&format!("# {}", doc.title)),
            "first chunk must carry the title header"
        );

        // Each chunk's char offset + char length must point into the augmented text.
        for chunk in &chunks {
            let start = chunk.chunk_offset as usize;
            let end = start + chunk.chunk_length as usize;
            assert!(
                end <= expected.chars().count(),
                "chunk [{start}..{end}] exceeds text length {}",
                expected.chars().count()
            );
        }

        // Concatenated chunk positions must not overlap and must be monotonic.
        for pair in chunks.windows(2) {
            let prev_end = pair[0].chunk_offset as usize + pair[0].chunk_length as usize;
            let next_start = pair[1].chunk_offset as usize;
            assert!(
                next_start >= prev_end,
                "chunks must not overlap: prev ends at {prev_end}, next starts at {next_start}"
            );
        }
    }

    #[test]
    fn markdown_headers_are_preserved_as_units() {
        let service = ChunkingService::new(80);
        let kb_id = make_kb_id();
        let doc = make_document("doc-1", kb_id, "Headers", "https://example.com/headers");

        let text = "## Section A\n\nContent under A.\n\n### Section B\n\nContent under B.";
        let chunks = service.chunk_markdown(kb_id, &doc, text);

        assert!(
            chunks
                .iter()
                .any(|c| c.content.contains("Section A") && c.content.contains("Content under A")),
            "Section A header must stay with its content when it fits the chunk size"
        );
        assert!(
            chunks
                .iter()
                .any(|c| c.content.contains("Section B") && c.content.contains("Content under B")),
            "Section B header must stay with its content when it fits"
        );
    }

    #[test]
    fn chunk_ids_are_unique() {
        let service = ChunkingService::new(10);
        let kb_id = make_kb_id();
        let doc = make_document("doc-1", kb_id, "Unique", "https://example.com/unique");

        let text = "Chunk one here. Chunk two there. Chunk three elsewhere. Chunk four somewhere.";
        let chunks = service.chunk_markdown(kb_id, &doc, text);

        let mut ids: Vec<&DocumentChunkId> = chunks.iter().map(|c| &c.id).collect();
        ids.sort_by(|a, b| a.0.cmp(&b.0));
        ids.dedup_by(|a, b| a.0 == b.0);
        assert_eq!(ids.len(), chunks.len(), "all chunk IDs must be unique");
    }

    #[test]
    fn document_metadata_is_propagated() {
        let service = ChunkingService::new(512);
        let kb_id = make_kb_id();
        let custom_meta = serde_json::json!({"author": "Alice", "lang": "en"});
        let doc = Document {
            id: DocumentId("doc-meta".to_string()),
            kb_id,
            title: "Meta Doc".to_string(),
            url: "https://example.com/meta".to_string(),
            version: "v3".to_string(),
            mime_type: MimeType::TextPlain,
            metadata: custom_meta.clone(),
            created_at: NaiveDateTime::MIN,
            updated_at: NaiveDateTime::MIN,
        };

        let chunks = service.chunk_markdown(kb_id, &doc, "some content");

        for chunk in &chunks {
            assert_eq!(chunk.title, "Meta Doc");
            assert_eq!(chunk.url, "https://example.com/meta");
            assert_eq!(chunk.version, "v3");
            assert_eq!(chunk.metadata, custom_meta);
        }
    }

    #[test]
    fn unicode_text_is_handled_correctly() {
        let service = ChunkingService::new(20);
        let kb_id = make_kb_id();
        let doc = make_document("doc-1", kb_id, "Unicode", "https://example.com/unicode");

        // Cyrillic text where each char is 2 bytes.
        let text = "Привет мир\n\nКак дела?";
        let expected = crate::utils::with_document_title(&doc.title, text).into_owned();
        let chunks = service.chunk_markdown(kb_id, &doc, text);

        assert!(!chunks.is_empty(), "unicode text must produce chunks");
        let char_starts: Vec<usize> = expected.char_indices().map(|(byte, _)| byte).collect();
        for chunk in &chunks {
            let start = chunk.chunk_offset;
            let end = start + chunk.chunk_length;
            let start_byte = char_starts[start];
            let end_byte = if end >= char_starts.len() {
                expected.len()
            } else {
                char_starts[end]
            };
            let slice = &expected[start_byte..end_byte];
            assert_eq!(
                slice, chunk.content,
                "chunk content at char offset {start} must match augmented text slice"
            );
        }
    }

    #[test]
    fn code_blocks_are_not_split_when_they_fit() {
        let service = ChunkingService::new(120);
        let kb_id = make_kb_id();
        let doc = make_document("doc-1", kb_id, "Code", "https://example.com/code");

        let text = "Intro paragraph.\n\n```rust\nfn main() {\n    println!(\"hello\");\n}\n```\n\nAfter code.";
        let chunks = service.chunk_markdown(kb_id, &doc, text);

        // The fenced code block should appear in full within some chunk.
        let code_chunk = chunks.iter().find(|c| c.content.contains("```rust"));
        assert!(code_chunk.is_some(), "code block must be preserved");
        let code_chunk = code_chunk.unwrap();
        assert!(
            code_chunk.content.contains("fn main()"),
            "code block content must be together: {}",
            code_chunk.content
        );
        assert!(
            code_chunk.content.contains("```"),
            "closing fence must stay with code block"
        );
    }
}
