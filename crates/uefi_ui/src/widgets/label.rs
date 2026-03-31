//! Single-line label (borrowed text for the frame).
//!
//! When drawing, use [`crate::theme::ThemeColors`] fields such as `text` and `text_secondary`.

/// Read-only caption; rendering uses your font pipeline and theme colors.
#[derive(Debug, Clone)]
pub struct Label<'a> {
    pub text: &'a str,
}

impl<'a> Label<'a> {
    pub const fn new(text: &'a str) -> Self {
        Self { text }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_holds_text() {
        let l = Label::new("hello");
        assert_eq!(l.text, "hello");
    }
}
