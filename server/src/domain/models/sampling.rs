/// Sampling parameters sent to the LLM request.
///
/// Every field is `None` unless explicitly set — unset fields are omitted from
/// the request so the provider default applies.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SamplingParams {
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub top_k: Option<u32>,
    pub min_p: Option<f32>,
    pub presence_penalty: Option<f32>,
    pub frequency_penalty: Option<f32>,
    pub repeat_penalty: Option<f32>,
    pub seed: Option<i64>,
}

impl SamplingParams {
    /// Field-by-field merge: each field from `overrides` wins over this value.
    pub fn merge(&self, overrides: &Self) -> Self {
        Self {
            temperature: overrides.temperature.or(self.temperature),
            top_p: overrides.top_p.or(self.top_p),
            top_k: overrides.top_k.or(self.top_k),
            min_p: overrides.min_p.or(self.min_p),
            presence_penalty: overrides.presence_penalty.or(self.presence_penalty),
            frequency_penalty: overrides.frequency_penalty.or(self.frequency_penalty),
            repeat_penalty: overrides.repeat_penalty.or(self.repeat_penalty),
            seed: overrides.seed.or(self.seed),
        }
    }

    /// `true` when no field is set.
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// Preset names referenced by scenarios.
pub struct SamplingPreset;

impl SamplingPreset {
    pub const THINKING: &str = "thinking";
    pub const INSTRUCT: &str = "instruct";
}

const THINKING: SamplingParams = SamplingParams {
    temperature: Some(1.0),
    top_p: Some(0.95),
    top_k: Some(20),
    min_p: Some(0.0),
    presence_penalty: Some(0.0),
    frequency_penalty: None,
    repeat_penalty: Some(1.0),
    seed: None,
};

const INSTRUCT: SamplingParams = SamplingParams {
    temperature: Some(0.7),
    top_p: Some(0.8),
    top_k: Some(20),
    min_p: Some(0.0),
    presence_penalty: Some(1.5),
    frequency_penalty: None,
    repeat_penalty: Some(1.0),
    seed: None,
};

/// Looks up a named sampling preset.
pub fn preset_by_name(name: &str) -> Option<SamplingParams> {
    match name {
        SamplingPreset::THINKING => Some(THINKING),
        SamplingPreset::INSTRUCT => Some(INSTRUCT),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{preset_by_name, SamplingParams, SamplingPreset};

    #[test]
    fn thinking_preset_values() {
        let p = preset_by_name(SamplingPreset::THINKING).unwrap();
        assert_eq!(p.temperature, Some(1.0));
        assert_eq!(p.top_p, Some(0.95));
        assert_eq!(p.top_k, Some(20));
        assert_eq!(p.min_p, Some(0.0));
        assert_eq!(p.presence_penalty, Some(0.0));
        assert_eq!(p.frequency_penalty, None);
        assert_eq!(p.repeat_penalty, Some(1.0));
        assert_eq!(p.seed, None);
        assert!(!p.is_empty());
    }

    #[test]
    fn instruct_preset_values() {
        let p = preset_by_name(SamplingPreset::INSTRUCT).unwrap();
        assert_eq!(p.temperature, Some(0.7));
        assert_eq!(p.top_p, Some(0.8));
        assert_eq!(p.top_k, Some(20));
        assert_eq!(p.min_p, Some(0.0));
        assert_eq!(p.presence_penalty, Some(1.5));
        assert_eq!(p.frequency_penalty, None);
        assert_eq!(p.repeat_penalty, Some(1.0));
        assert_eq!(p.seed, None);
        assert!(!p.is_empty());
    }

    #[test]
    fn unknown_preset_name_returns_none() {
        assert!(preset_by_name("").is_none());
        assert!(preset_by_name("thinking-2").is_none());
    }

    #[test]
    fn merge_overrides_win_field_by_field() {
        let base = preset_by_name(SamplingPreset::INSTRUCT).unwrap();
        let overrides = SamplingParams {
            temperature: Some(0.1),
            top_p: None,
            top_k: None,
            min_p: None,
            presence_penalty: None,
            frequency_penalty: Some(0.5),
            repeat_penalty: None,
            seed: Some(7),
        };
        let merged = base.merge(&overrides);
        assert_eq!(merged.temperature, Some(0.1));
        assert_eq!(merged.top_p, base.top_p);
        assert_eq!(merged.top_k, base.top_k);
        assert_eq!(merged.min_p, base.min_p);
        assert_eq!(merged.presence_penalty, base.presence_penalty);
        assert_eq!(merged.frequency_penalty, Some(0.5));
        assert_eq!(merged.repeat_penalty, base.repeat_penalty);
        assert_eq!(merged.seed, Some(7));
    }

    #[test]
    fn default_merge_keeps_base() {
        let base = preset_by_name(SamplingPreset::THINKING).unwrap();
        assert_eq!(base.merge(&SamplingParams::default()), base);
    }

    #[test]
    fn default_is_empty() {
        assert!(SamplingParams::default().is_empty());
    }
}
