use crate::domain::models::chat_search_config::SearchMode;
use crate::errors::AppError;

/// Per-request search limits and thresholds. `limit` is applied per knowledge
/// base, the other two per (query, knowledge base). `rerank: false` skips the
/// reranker: chunks keep their hybrid (RRF) score and are merged by dedup only.
/// `mode` selects the retrieval leg(s): hybrid (dense + BM25), semantic (dense
/// only) or lexical (BM25 only).
#[derive(Clone, Copy, Debug)]
pub struct SearchParams {
    pub limit: usize,
    pub candidate_limit: u64,
    pub reranker_limit: u32,
    pub reranker_threshold: f32,
    pub rerank: bool,
    pub mode: SearchMode,
}

impl SearchParams {
    pub fn try_new(
        limit: usize,
        candidate_limit: u64,
        reranker_limit: u32,
        reranker_threshold: f32,
        rerank: bool,
    ) -> Result<Self, AppError> {
        if limit == 0 {
            return Err(AppError::BadRequest(
                "search limit must be at least 1".into(),
            ));
        }
        if candidate_limit == 0 {
            return Err(AppError::BadRequest(
                "search candidate_limit must be at least 1".into(),
            ));
        }
        if reranker_limit == 0 {
            return Err(AppError::BadRequest(
                "search reranker_limit must be at least 1".into(),
            ));
        }
        if !(reranker_threshold >= 0.0 && reranker_threshold <= 1.0) {
            return Err(AppError::BadRequest(
                "search reranker_threshold must be within 0.0..=1.0".into(),
            ));
        }
        Ok(Self {
            limit,
            candidate_limit,
            reranker_limit,
            reranker_threshold,
            rerank,
            mode: SearchMode::default(),
        })
    }

    pub fn with_mode(mut self, mode: SearchMode) -> Self {
        self.mode = mode;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_construction() {
        let params = SearchParams::try_new(15, 30, 7, 0.5, true).unwrap();
        assert_eq!(params.limit, 15);
        assert_eq!(params.candidate_limit, 30);
        assert_eq!(params.reranker_limit, 7);
        assert_eq!(params.reranker_threshold, 0.5);
    }

    #[test]
    fn threshold_boundaries_are_accepted() {
        assert!(SearchParams::try_new(1, 1, 1, 0.0, true).is_ok());
        assert!(SearchParams::try_new(1, 1, 1, 1.0, true).is_ok());
    }

    #[test]
    fn limit_zero_rejected() {
        let err = SearchParams::try_new(0, 30, 7, 0.5, true).expect_err("limit 0 must be rejected");
        assert!(matches!(err, AppError::BadRequest(_)));
        assert!(err.to_string().contains("limit"));
    }

    #[test]
    fn candidate_limit_zero_rejected() {
        let err = SearchParams::try_new(15, 0, 7, 0.5, true)
            .expect_err("candidate_limit 0 must be rejected");
        assert!(matches!(err, AppError::BadRequest(_)));
        assert!(err.to_string().contains("candidate_limit"));
    }

    #[test]
    fn reranker_limit_zero_rejected() {
        let err = SearchParams::try_new(15, 30, 0, 0.5, true)
            .expect_err("reranker_limit 0 must be rejected");
        assert!(matches!(err, AppError::BadRequest(_)));
        assert!(err.to_string().contains("reranker_limit"));
    }

    #[test]
    fn threshold_out_of_range_rejected() {
        assert!(SearchParams::try_new(15, 30, 7, -0.1, true).is_err());
        assert!(SearchParams::try_new(15, 30, 7, 1.1, true).is_err());
    }

    #[test]
    fn threshold_nan_rejected() {
        assert!(SearchParams::try_new(15, 30, 7, f32::NAN, true).is_err());
    }
}
