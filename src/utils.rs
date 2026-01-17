use std::path::Path;

pub fn is_markdown(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("md"))
        .unwrap_or(false)
}

pub fn trim_line(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

pub fn pluralize(word: &str, count: usize) -> String {
    pluralize_with(word, count, |n| n.to_string())
}

pub fn pluralize_with<F>(word: &str, count: usize, format_count: F) -> String
where
    F: Fn(usize) -> String,
{
    let count_str = format_count(count);

    if count == 1 {
        format!("{count_str} {word}")
    } else {
        format!("{count_str} {word}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_markdown() {
        assert!(is_markdown(Path::new("test.md")));
        assert!(!is_markdown(Path::new("test.txt")));
    }

    #[test]
    fn test_pluralize_single() {
        assert_eq!(pluralize("card", 1), "1 card");
        assert_eq!(pluralize("cloze card", 1), "1 cloze card");
    }

    #[test]
    fn test_pluralize_multiple() {
        assert_eq!(pluralize("card", 2), "2 cards");
        assert_eq!(pluralize("card", 5), "5 cards");
        assert_eq!(pluralize("cloze card", 3), "3 cloze cards");
    }

    #[test]
    fn test_pluralize_zero() {
        assert_eq!(pluralize("card", 0), "0 cards");
    }
}
