use phf::phf_set;

use crate::utils::trim_line;

static STOPWORDS: phf::Set<&'static str> = phf_set! {"i", "me", "my", "myself", "we", "our", "ours", "ourselves", "you", "your", "yours", "yourself", "yourselves", "he", "him", "his", "himself", "she", "her", "hers", "herself", "it", "its", "itself", "they", "them", "their", "theirs", "themselves", "what", "which", "who", "whom", "this", "that", "these", "those", "am", "is", "are", "was", "were", "be", "been", "being", "have", "has", "had", "having", "do", "does", "did", "doing", "a", "an", "the", "and", "but", "if", "or", "because", "as", "until", "while", "of", "at", "by", "for", "with", "about", "against", "between", "into", "through", "during", "before", "after", "above", "below", "to", "from", "up", "down", "in", "out", "on", "off", "over", "under", "again", "further", "then", "once", "here", "there", "when", "where", "why", "how", "all", "any", "both", "each", "few", "more", "most", "other", "some", "such", "no", "nor", "not", "only", "own", "same", "so", "than", "too", "very", "s", "t", "can", "will", "just", "don", "should", "now"};

pub fn get_hash(s: &str) -> Option<String> {
    trim_line(s)?;
    let mut hasher = blake3::Hasher::new();

    // ===== ASCII FAST PATH =====
    if s.is_ascii() {
        let mut word = Vec::<u8>::with_capacity(16);

        for &b in s.as_bytes() {
            match b {
                b'A'..=b'Z' => word.push(b + 32), // lowercase
                b'a'..=b'z' | b'0'..=b'9' => word.push(b),

                // Joiners: ignored, do NOT flush
                b'\'' => {}

                b'+' | b'-' => {
                    flush_ascii_word(&mut hasher, &mut word);
                    hasher.update(&[b]);
                }
                _ => {
                    flush_ascii_word(&mut hasher, &mut word);
                }
            }
        }

        flush_ascii_word(&mut hasher, &mut word);
        return return_hasher(hasher);
    }

    // ===== UNICODE FALLBACK =====
    unicode_path(s, &mut hasher);
    return_hasher(hasher)
}

fn return_hasher(hasher: blake3::Hasher) -> Option<String> {
    if hasher.count() == 0 {
        None
    } else {
        Some(hasher.finalize().to_string())
    }
}

fn flush_ascii_word(hasher: &mut blake3::Hasher, word: &mut Vec<u8>) {
    if word.is_empty() {
        return;
    }

    // SAFETY: ASCII-only bytes
    let w = unsafe { std::str::from_utf8_unchecked(word) };

    if !STOPWORDS.contains(w) {
        hasher.update(word);
    }

    word.clear();
}

fn unicode_path(s: &str, hasher: &mut blake3::Hasher) {
    let mut word = String::new();

    for ch in s.chars() {
        if matches!(ch, '\'' | '’' | 'ʼ' | 'ʻ' | '‛' | '＇') {
            continue;
        }
        if ch == '+' || ch == '-' {
            flush_unicode_word(hasher, &mut word);
            hasher.update(&[ch as u8]);
            continue;
        }

        if ch.is_alphanumeric() {
            for lc in ch.to_lowercase() {
                word.push(lc);
            }
        } else {
            flush_unicode_word(hasher, &mut word);
        }
    }

    flush_unicode_word(hasher, &mut word);
}

fn flush_unicode_word(hasher: &mut blake3::Hasher, word: &mut String) {
    if word.is_empty() {
        return;
    }

    if !STOPWORDS.contains(word.as_str()) {
        hasher.update(word.as_bytes());
    }

    word.clear();
}
#[cfg(test)]
mod tests {
    use crate::hash::get_hash;
    use proptest::prelude::*;
    proptest! {
        #[test]
        fn test_card_parser( content in "\\PC*") {
            get_hash(&content);
        }
    }

    #[test]
    fn test_hash() {
        let a = "Hello,  world.\nIt's  2+2 - 1.";
        let b = "hello world its 2+2-1";
        let c = "  HELLO\tWORLD\tIT'S\t2+2 - 1  ";
        let d = "hello world 2+2-1";

        let ha = get_hash(a);
        let hb = get_hash(b);
        let hc = get_hash(c);
        let hd = get_hash(d);

        assert_eq!(ha, hb);
        assert_eq!(ha, hc);
        assert_eq!(ha, hd);
    }

    #[test]
    fn invalid_hash() {
        let a = "this that";
        let ha = get_hash(a);
        assert!(ha.is_none());

        let a = "this that science";
        let ha = get_hash(a);
        assert!(ha.is_some());
    }
}
