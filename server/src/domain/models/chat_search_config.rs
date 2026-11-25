use serde::{Deserialize, Serialize};

use crate::domain::models::search_params::SearchParams;
use crate::errors::AppError;

/// Search retrieval mode for a chat's KB searches.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchMode {
    /// Dense + BM25 prefetches fused with RRF (the default).
    #[default]
    Hybrid,
    /// Dense (semantic) vector search only.
    Semantic,
    /// BM25 (lexical) search only.
    Lexical,
}

/// Per-chat search overrides. Every field is optional; unset fields fall back
/// to the server-wide `search.*` configuration defaults.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChatSearchConfig {
    pub mode: Option<SearchMode>,
    pub limit: Option<usize>,
    pub candidate_limit: Option<u64>,
    pub reranker_limit: Option<u32>,
    pub reranker_threshold: Option<f32>,
    pub rerank: Option<bool>,
}

impl ChatSearchConfig {
    /// Merge this chat's overrides on top of the server defaults, keeping the
    /// existing range validation of `SearchParams::try_new`.
    pub fn resolve(&self, base: &SearchParams) -> Result<SearchParams, AppError> {
        SearchParams::try_new(
            self.limit.unwrap_or(base.limit),
            self.candidate_limit.unwrap_or(base.candidate_limit),
            self.reranker_limit.unwrap_or(base.reranker_limit),
            self.reranker_threshold.unwrap_or(base.reranker_threshold),
            self.rerank.unwrap_or(base.rerank),
        )
        .map(|params| params.with_mode(self.mode.unwrap_or(base.mode)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> SearchParams {
        SearchParams::try_new(15, 20, 7, 0.5, true).expect("valid base params")
    }

    #[test]
    fn resolve_with_no_overrides_equals_base() {
        let base = base();
        let resolved = ChatSearchConfig::default().resolve(&base).expect("valid");
        assert_eq!(resolved.limit, base.limit);
        assert_eq!(resolved.candidate_limit, base.candidate_limit);
        assert_eq!(resolved.reranker_limit, base.reranker_limit);
        assert_eq!(resolved.reranker_threshold, base.reranker_threshold);
        assert_eq!(resolved.rerank, base.rerank);
        assert_eq!(resolved.mode, base.mode);
    }

    #[test]
    fn resolve_applies_overrides() {
        let base = base();
        let cfg = ChatSearchConfig {
            mode: Some(SearchMode::Lexical),
            limit: Some(5),
            rerank: Some(false),
            ..Default::default()
        };
        let resolved = cfg.resolve(&base).expect("valid");
        assert_eq!(resolved.mode, SearchMode::Lexical);
        assert_eq!(resolved.limit, 5);
        assert_eq!(resolved.candidate_limit, base.candidate_limit);
        assert!(!resolved.rerank);
        assert_eq!(resolved.reranker_threshold, base.reranker_threshold);
    }

    #[test]
    fn resolve_rejects_invalid_override() {
        let base = base();
        let cfg = ChatSearchConfig {
            limit: Some(0),
            ..Default::default()
        };
        assert!(cfg.resolve(&base).is_err());
    }

    #[test]
    fn resolve_rejects_threshold_out_of_range() {
        let base = base();
        let cfg = ChatSearchConfig {
            reranker_threshold: Some(1.5),
            ..Default::default()
        };
        assert!(cfg.resolve(&base).is_err());
    }
}
