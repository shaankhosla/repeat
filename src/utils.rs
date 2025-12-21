use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};

use crate::card::{Card, CardContent};

pub fn validate_file_can_be_card(path: String) -> Result<PathBuf> {
    let card_path = trim_line(&path).ok_or_else(|| anyhow!("Card path cannot be empty"))?;
    let card_path = PathBuf::from(card_path);
    if card_path.is_dir() {
        return Err(anyhow!(
            "Card path cannot be a directory: {}",
            card_path.display()
        ));
    }

    if !is_markdown(&card_path) {
        return Err(anyhow!(
            "Card path must be a markdown file: {}",
            card_path.display()
        ));
    }

    Ok(card_path)
}
pub fn is_markdown(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("md"))
        .unwrap_or(false)
}

fn find_cloze_ranges(text: &str) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let mut start = None;

    for (i, ch) in text.char_indices() {
        match ch {
            '[' if start.is_none() => start = Some(i),
            ']' if start.is_some() => {
                let s = start.take().unwrap();
                let e = i;
                ranges.push((s, e));
            }
            _ => {}
        }
    }

    ranges
}
pub fn trim_line(line: &str) -> Option<String> {
    let trimmed_line = line.trim().to_string();
    if trimmed_line.is_empty() {
        return None;
    }
    Some(trimmed_line)
}
fn parse_card_lines(contents: &str) -> (Option<String>, Option<String>, Option<String>) {
    let mut question_lines = Vec::new();
    let mut answer_lines = Vec::new();
    let mut cloze_lines = Vec::new();

    enum Section {
        Question,
        Answer,
        Cloze,
        None,
    }

    let mut section = Section::None;

    for raw_line in contents.lines() {
        let line = match trim_line(raw_line) {
            Some(line) => line,
            None => continue,
        };

        if let Some(rest) = line.strip_prefix("Q:") {
            section = Section::Question;
            question_lines.clear();
            if let Some(q) = trim_line(rest) {
                question_lines.push(q)
            }
            continue;
        } else if let Some(rest) = line.strip_prefix("A:") {
            section = Section::Answer;
            answer_lines.clear();
            if let Some(q) = trim_line(rest) {
                answer_lines.push(q)
            }
            continue;
        } else if let Some(rest) = line.strip_prefix("C:") {
            section = Section::Cloze;
            cloze_lines.clear();
            if let Some(q) = trim_line(rest) {
                cloze_lines.push(q)
            }
            continue;
        }

        match section {
            Section::Question => question_lines.push(line.to_string()),
            Section::Answer => answer_lines.push(line.to_string()),
            Section::Cloze => cloze_lines.push(line.to_string()),
            Section::None => {}
        }
    }

    let join_nonempty = |v: Vec<String>| {
        if v.is_empty() {
            None
        } else {
            Some(v.join("\n"))
        }
    };

    (
        join_nonempty(question_lines),
        join_nonempty(answer_lines),
        join_nonempty(cloze_lines),
    )
}
pub fn content_to_card(
    card_path: &Path,
    contents: &str,
    file_start_idx: usize,
    file_end_idx: usize,
) -> Result<Card> {
    let (question, answer, cloze) = parse_card_lines(contents);

    let card_hash = get_hash(contents).ok_or_else(|| anyhow!("Unable to hash contents"))?;
    if let (Some(q), Some(a)) = (question, answer) {
        let content = CardContent::Basic {
            question: q,
            answer: a,
        };
        Ok(Card {
            file_path: card_path.to_path_buf(),
            file_card_range: (file_start_idx, file_end_idx),
            content,
            card_hash,
        })
    } else if let Some(c) = cloze {
        let cloze_idxs = find_cloze_ranges(&c);
        if cloze_idxs.is_empty() {
            return Err(anyhow!("Card is a cloze but can't find cloze text in []"));
        }
        let cloze_idx_start = cloze_idxs[0].0;
        let cloze_idx_end = cloze_idxs[0].1;
        if cloze_idx_end - cloze_idx_start <= 1 {
            return Err(anyhow!("Card is a cloze but can't find cloze text in []"));
        }
        let content = CardContent::Cloze {
            text: c,
            start: cloze_idx_start,
            end: cloze_idx_end,
        };
        Ok(Card {
            file_path: card_path.to_path_buf(),
            file_card_range: (file_start_idx, file_end_idx),
            content,
            card_hash,
        })
    } else {
        Err(anyhow!("Unable to create card: {}", card_path.display()))
    }
}

pub fn get_hash(content: &str) -> Option<String> {
    if let Some(content) = trim_line(content) {
        return Some(blake3::hash(content.as_bytes()).to_string());
    }
    None
}

pub fn cards_from_md(path: &Path) -> Result<Vec<Card>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);

    let mut cards = Vec::new();
    let mut buffer = String::new();
    let mut start_idx = 0;
    let mut last_idx = 0;
    for (idx, line) in reader.lines().enumerate() {
        let line = line?;
        if line.starts_with("Q:") || line.starts_with("C:") {
            if !buffer.is_empty() {
                cards.push(content_to_card(path, &buffer, start_idx, idx)?);
                buffer.clear();
            }
            start_idx = idx;
        }
        buffer.push_str(&line);
        buffer.push('\n');
        last_idx = idx;
    }
    if !buffer.is_empty() {
        cards.push(content_to_card(path, &buffer, start_idx, last_idx + 1)?);
    }

    Ok(cards)
}

fn collect_markdown_files(path: &Path, acc: &mut Vec<PathBuf>) -> Result<()> {
    if path.is_file() {
        if is_markdown(path) {
            acc.push(path.to_path_buf());
        }
        return Ok(());
    }

    if path.is_dir() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let entry_path = entry.path();
            if entry.file_type()?.is_dir() {
                collect_markdown_files(&entry_path, acc)?;
            } else if is_markdown(&entry_path) {
                acc.push(entry_path);
            }
        }
        return Ok(());
    }

    Err(anyhow!("Path does not exist: {}", path.display()))
}

pub fn cards_from_files(paths: &[PathBuf]) -> Result<Vec<Card>> {
    let mut cards = Vec::new();
    for path in paths {
        cards.extend(cards_from_md(path)?);
    }
    Ok(cards)
}

pub fn cards_from_dir(path: &Path) -> Result<Vec<Card>> {
    let mut markdown_files = Vec::new();
    collect_markdown_files(path, &mut markdown_files)?;
    markdown_files.sort();

    cards_from_files(&markdown_files)
}

#[cfg(test)]
mod tests {
    use crate::utils::{cards_from_dir, content_to_card, parse_card_lines};
    use std::path::PathBuf;

    use crate::card::CardContent;

    use super::cards_from_md;

    #[test]
    fn test_card_parsing() {
        let contents = "C:\nRegion: [`us-east-2`]\n\nLocation: [Ohio]\n\n---\n\n";
        let (question, _, cloze) = parse_card_lines(contents);
        assert!(question.is_none());
        assert_eq!(
            "Region: [`us-east-2`]\nLocation: [Ohio]\n---",
            cloze.unwrap()
        );
    }

    #[test]
    fn basic_qa() {
        let card_path = PathBuf::from("test.md");

        let card = content_to_card(&card_path, "", 1, 1);
        assert!(card.is_err());

        let card = content_to_card(&card_path, "what am i doing here", 1, 1);
        assert!(card.is_err());

        let content = "Q: what?\nA: yes\n\n";
        let card = content_to_card(&card_path, content, 1, 1);
        if let CardContent::Basic { question, answer } = &card.expect("should be basic").content {
            assert_eq!(question, "what?");
            assert_eq!(answer, "yes");
        } else {
            panic!("Expected CardContent::Basic");
        }

        let content = "Q: what?\nA: \n\n";
        let card = content_to_card(&card_path, content, 1, 1);
        assert!(card.is_err());
    }

    #[test]
    fn basic_cloze() {
        let card_path = PathBuf::from("test.md");

        let content = "C: ping? [pong]";
        let card = content_to_card(&card_path, content, 1, 1);
        if let CardContent::Cloze { text, start, end } = &card.expect("should be basic").content {
            assert_eq!(text, "ping? [pong]");
            assert_eq!(*start, 6_usize);
            assert_eq!(*end, 11_usize);
        } else {
            panic!("Expected CardContent::Cloze");
        }
    }

    #[test]
    fn test_file_capture() {
        let card_path = PathBuf::from("test_data/test.md");
        let cards = cards_from_md(&card_path).expect("should be ok");

        assert_eq!(cards.len(), 4);
    }

    #[test]
    fn collects_cards_from_directory() {
        let dir_path = PathBuf::from("test_data");
        let cards = cards_from_dir(&dir_path).expect("should collect cards");
        assert_eq!(cards.len(), 4);
        assert!(
            cards
                .iter()
                .all(|card| card.file_path.ends_with("test_data/test.md"))
        );
    }
}
