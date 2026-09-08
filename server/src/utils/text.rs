use std::borrow::Cow;

pub fn with_document_title<'a>(title: &str, text: &'a str) -> Cow<'a, str> {
    let title = title.trim();
    if title.is_empty() {
        return Cow::Borrowed(text);
    }

    let mut augmented = String::with_capacity(title.len() + 2 + text.len());
    augmented.push_str("# ");
    augmented.push_str(title);
    augmented.push_str("\n\n");
    augmented.push_str(text);
    Cow::Owned(augmented)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_title_returns_text_unchanged() {
        let text = "body";
        assert_eq!(with_document_title("", text), text);
        assert_eq!(with_document_title("   ", text), text);
    }

    #[test]
    fn empty_text_with_title_produces_just_header() {
        assert_eq!(with_document_title("Doc", ""), "# Doc\n\n");
    }

    #[test]
    fn prepends_title_as_first_level_heading() {
        assert_eq!(
            with_document_title("Some title", "body text"),
            "# Some title\n\nbody text"
        );
    }

    #[test]
    fn title_is_trimmed() {
        assert_eq!(
            with_document_title("  Some title  ", "body"),
            "# Some title\n\nbody"
        );
    }
}
