use serde::{Deserialize, Serialize};

/// Token accounting for a set of LLM calls (a round or a whole scenario).
///
/// `cached_prompt_tokens` is the part of the prompt served from the provider's
/// prompt cache; it is 0 when the provider does not report it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenUsage {
    #[serde(default)]
    pub prompt_tokens: u32,
    #[serde(default)]
    pub completion_tokens: u32,
    #[serde(default)]
    pub total_tokens: u32,
    #[serde(default)]
    pub cached_prompt_tokens: u32,
}

impl TokenUsage {
    pub fn add(&mut self, other: &TokenUsage) {
        self.prompt_tokens += other.prompt_tokens;
        self.completion_tokens += other.completion_tokens;
        self.total_tokens += other.total_tokens;
        self.cached_prompt_tokens += other.cached_prompt_tokens;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn usage(prompt: u32, completion: u32, total: u32, cached: u32) -> TokenUsage {
        TokenUsage {
            prompt_tokens: prompt,
            completion_tokens: completion,
            total_tokens: total,
            cached_prompt_tokens: cached,
        }
    }

    #[test]
    fn add_sums_all_counters() {
        let mut a = usage(10, 5, 15, 8);
        a.add(&usage(3, 2, 5, 1));
        assert_eq!(a, usage(13, 7, 20, 9));
    }

    #[test]
    fn serde_is_camel_case() {
        let json = serde_json::to_string(&usage(1, 2, 3, 4)).unwrap();
        assert_eq!(
            json,
            "{\"promptTokens\":1,\"completionTokens\":2,\"totalTokens\":3,\"cachedPromptTokens\":4}"
        );
    }

    #[test]
    fn serde_round_trips() {
        let u = usage(1, 2, 3, 4);
        let json = serde_json::to_string(&u).unwrap();
        assert_eq!(serde_json::from_str::<TokenUsage>(&json).unwrap(), u);
    }

    #[test]
    fn missing_fields_default_to_zero() {
        let u: TokenUsage = serde_json::from_str("{}").unwrap();
        assert_eq!(u, TokenUsage::default());
    }
}
