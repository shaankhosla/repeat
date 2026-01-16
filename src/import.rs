use html_escape::decode_html_entities;
use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::Value;
use sqlx::{Row, SqlitePool};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;
use zip::ZipArchive;

use anyhow::{Context, Result, anyhow, bail};

use crate::crud::DB;
use crate::palette::Palette;
use crate::parser::get_hash;

static TAG_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?is)<[^>]+>").unwrap());
static CLOZE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?s)\{\{c\d+::(.*?)(?:::(.*?))?\}\}").unwrap());

#[derive(Clone)]
struct DeckInfo {
    name: String,
    components: Vec<String>,
}

#[derive(Clone, Copy)]
enum ModelKind {
    Basic,
    Cloze,
}

#[derive(Clone, Debug)]
struct CardRecord {
    deck_id: i64,
    model_id: i64,
    card_order: i64,
    fields: Vec<String>,
}

pub async fn run(_db: &DB, anki_path: &Path, export_path: &Path) -> Result<()> {
    validate_path(anki_path)?;
    let db_path = extract_collection_db(anki_path)?;
    let db_url = format!("sqlite://{}", db_path.path().display());
    let export_db = SqlitePool::connect(&db_url)
        .await
        .context("failed to connect to Anki database")?;
    let (decks, models) = load_metadata(&export_db).await?;
    let cards = load_cards(&export_db).await?;
    let exports = build_exports(cards, &models);
    write_exports(export_path, &decks, exports)?;
    Ok(())
}

fn validate_path(anki_path: &Path) -> Result<()> {
    if !anki_path.exists() {
        bail!("Anki path does not exist: {}", anki_path.display());
    }
    if !anki_path.is_file() || anki_path.extension() != Some("apkg".as_ref()) {
        bail!(
            "Anki path does not point to an apkg file: {}",
            anki_path.display()
        );
    }
    Ok(())
}

fn extract_collection_db(apkg: &Path) -> Result<NamedTempFile> {
    let file = File::open(apkg)
        .with_context(|| format!("failed to open apkg file: {}", apkg.display()))?;

    let mut zip = ZipArchive::new(file).context("failed to read apkg as zip archive")?;

    let mut entry = {
        if let Ok(e) = zip.by_name("collection.anki21") {
            e
        } else {
            zip.by_name("collection.anki2").context(
                "apkg does not contain the newer collection.anki21 or the older collection.anki2",
            )?
        }
    };

    let mut temp =
        NamedTempFile::new().context("failed to create temporary file for sqlite database")?;

    std::io::copy(&mut entry, &mut temp).context("failed to extract collection.anki2 from apkg")?;

    Ok(temp)
}

async fn load_metadata(
    pool: &SqlitePool,
) -> Result<(HashMap<i64, DeckInfo>, HashMap<i64, ModelKind>)> {
    let row = sqlx::query("SELECT decks, models FROM col LIMIT 1")
        .fetch_one(pool)
        .await
        .context("failed to read deck metadata")?;
    let decks_raw: String = row.try_get("decks")?;
    let models_raw: String = row.try_get("models")?;
    let decks = parse_decks(&decks_raw)?;
    let models = parse_models(&models_raw)?;
    println!(
        "Found {} decks and {} models in DB schema",
        Palette::paint(Palette::WARNING, decks.len()),
        Palette::paint(Palette::WARNING, models.len())
    );
    Ok((decks, models))
}

// {"1736956380790":{"id":1736956380790,"mod":1738946904,"name":"Data Science::metrics","usn":-1,"lrnToday":[23,0],"revToday":[23,4],"newToday":[23,0],"timeToday":[23,79178],"collapsed":false,"browserCollapsed":false,"desc":"","dyn":0,"conf":1,"extendNew":0,"extendRev":0,"reviewLimit":null,"newLimit":null,"reviewLimitToday":null,"newLimitToday":null},"1736956380787":{"id":1736956380787,"mod":1738460183,"name":"Data Science::stats","usn":-1,"lrnToday":[17,0],"revToday":[17,5],"newToday":[17,0],"timeToday":[17,56853],"collapsed":false,"browserCollapsed":false,"desc":"","dyn":0,"conf":1,"extendNew":0,"extendRev":0,"reviewLimit":null,"newLimit":null,"reviewLimitToday":null,"newLimitToday":null},"1736956380788":{"id":1736956380788,"mod":1738946986,"name":"Data Science::supervised","usn":-1,"lrnToday":[23,0],"revToday":[23,5],"newToday":[23,0],"timeToday":[23,58570],"collapsed":false,"browserCollapsed":false,"desc":"","dyn":0,"conf":1,"extendNew":0,"extendRev":0,"reviewLimit":null,"newLimit":null,"reviewLimitToday":null,"newLimitToday":null},"1736956380789":{"id":1736956380789,"mod":1767018023,"name":"Data Science::clustering","usn":-1,"lrnToday":[348,0],"revToday":[348,2],"newToday":[348,0],"timeToday":[348,17276],"collapsed":false,"browserCollapsed":false,"desc":"","dyn":0,"conf":1,"extendNew":0,"extendRev":0,"reviewLimit":null,"newLimit":null,"reviewLimitToday":null,"newLimitToday":null},"1736956380791":{"id":1736956380791,"mod":1738946978,"name":"Data Science::linear-algebra","usn":-1,"lrnToday":[23,0],"revToday":[23,3],"newToday":[23,0],"timeToday":[23,37053],"collapsed":false,"browserCollapsed":false,"desc":"","dyn":0,"conf":1,"extendNew":0,"extendRev":0,"reviewLimit":null,"newLimit":null,"reviewLimitToday":null,"newLimitToday":null},"1736956380792":{"id":1736956380792,"mod":1738946547,"name":"Data Science::feature-engineering","usn":-1,"lrnToday":[23,0],"revToday":[23,0],"newToday":[23,0],"timeToday":[23,29170],"collapsed":false,"browserCollapsed":false,"desc":"","dyn":0,"conf":1,"extendNew":0,"extendRev":0,"reviewLimit":null,"newLimit":null,"reviewLimitToday":null,"newLimitToday":null},"1736956380786":{"id":1736956380786,"mod":1767018023,"name":"Data Science","usn":-1,"lrnToday":[348,0],"revToday":[348,2],"newToday":[348,0],"timeToday":[348,17276],"collapsed":false,"browserCollapsed":false,"desc":"Please see the <a href='https://ankiweb.net/shared/info/1443276573'>shared deck page</a> for more info.","dyn":0,"conf":1,"extendNew":0,"extendRev":0,"reviewLimit":null,"newLimit":null,"reviewLimitToday":null,"newLimitToday":null},"1":{"id":1,"mod":0,"name":"Default","usn":0,"lrnToday":[0,0],"revToday":[0,0],"newToday":[0,0],"timeToday":[0,0],"collapsed":true,"browserCollapsed":true,"desc":"","dyn":0,"conf":1,"extendNew":0,"extendRev":0,"reviewLimit":null,"newLimit":null,"reviewLimitToday":null,"newLimitToday":null}}
fn parse_decks(json: &str) -> Result<HashMap<i64, DeckInfo>> {
    let value: Value = serde_json::from_str(json).context("failed to parse decks json")?;
    let mut decks = HashMap::new();
    if let Some(map) = value.as_object() {
        for deck in map.values() {
            if let Some(id) = deck.get("id").and_then(|v| v.as_i64()) {
                // name could be Data Science::clustering
                let name = deck.get("name").and_then(|v| v.as_str()).unwrap_or("Deck");
                decks.insert(
                    id,
                    DeckInfo {
                        name: name.to_string(),
                        components: deck_components(name),
                    },
                );
            }
        }
    }
    Ok(decks)
}

// {"1736956091574":{"id":1736956091574,"name":"Basic","type":0,"mod":0,"usn":0,"sortf":0,"did":null,"tmpls":[{"name":"Card 1","ord":0,"qfmt":"{{Front}}","afmt":"{{FrontSide}}\n\n<hr id=answer>\n\n{{Back}}","bqfmt":"","bafmt":"","did":null,"bfont":"","bsize":0,"id":-7582984624314878559}],"flds":[{"name":"Front","ord":0,"sticky":false,"rtl":false,"font":"Arial","size":20,"description":"","plainText":false,"collapsed":false,"excludeFromSearch":false,"id":9187657064927433214,"tag":null,"preventDeletion":false},{"name":"Back","ord":1,"sticky":false,"rtl":false,"font":"Arial","size":20,"description":"","plainText":false,"collapsed":false,"excludeFromSearch":false,"id":7126803600113827766,"tag":null,"preventDeletion":false}],"css":".card {\n    font-family: arial;\n    font-size: 20px;\n    text-align: center;\n    color: black;\n    background-color: white;\n}\n","latexPre":"\\documentclass[12pt]{article}\n\\special{papersize=3in,5in}\n\\usepackage[utf8]{inputenc}\n\\usepackage{amssymb,amsmath}\n\\pagestyle{empty}\n\\setlength{\\parindent}{0in}\n\\begin{document}\n","latexPost":"\\end{document}","latexsvg":false,"req":[[0,"any",[0]]],"originalStockKind":1},"1493859779912":{"id":1493859779912,"name":"Basic-DataSci","type":0,"mod":1601179117,"usn":-1,"sortf":0,"did":1535429881193,"tmpls":[{"name":"Card 1","ord":0,"qfmt":"{{Front}}","afmt":"{{FrontSide}}\n\n<hr id=answer>\n\n{{Back}}\n\n{{#Ref}}\n<br/>\n<a href=\"{{text:Ref}}\">Ref</a>\n{{/Ref}}","bqfmt":"","bafmt":"","did":null,"bfont":"","bsize":0,"id":null}],"flds":[{"name":"Front","ord":0,"sticky":false,"rtl":false,"font":"Liberation Sans","size":20,"description":"","plainText":false,"collapsed":false,"excludeFromSearch":false,"id":null,"tag":null,"preventDeletion":false,"media":[]},{"name":"Back","ord":1,"sticky":false,"rtl":false,"font":"Arial","size":20,"description":"","plainText":false,"collapsed":false,"excludeFromSearch":false,"id":null,"tag":null,"preventDeletion":false,"media":[]},{"name":"Ref","ord":2,"sticky":false,"rtl":false,"font":"Liberation Sans","size":20,"description":"","plainText":false,"collapsed":false,"excludeFromSearch":false,"id":null,"tag":null,"preventDeletion":false,"media":[]},{"name":"Credit","ord":3,"sticky":false,"rtl":false,"font":"Liberation Sans","size":20,"description":"","plainText":false,"collapsed":false,"excludeFromSearch":false,"id":null,"tag":null,"preventDeletion":false,"media":[]}],"css":".card {\n font-family: arial;\n font-size: 20px;\n text-align: center;\n color: black;\n background-color: white;\n}\n","latexPre":"\\documentclass[12pt]{article}\n\\special{papersize=3in,5in}\n\\usepackage[utf8]{inputenc}\n\\usepackage{amssymb,amsmath}\n\\pagestyle{empty}\n\\setlength{\\parindent}{0in}\n\\begin{document}\n","latexPost":"\\end{document}","latexsvg":false,"req":[[0,"any",[0]]],"originalId":1493859779912,"prewrap":false,"tags":["low-level"],"vers":[]}}
fn parse_models(json: &str) -> Result<HashMap<i64, ModelKind>> {
    let value: Value = serde_json::from_str(json).context("failed to parse models json")?;
    let mut models = HashMap::new();
    if let Some(map) = value.as_object() {
        for model in map.values() {
            if let Some(id) = model.get("id").and_then(|v| v.as_i64()) {
                let kind = match model.get("type").and_then(|v| v.as_i64()).unwrap_or(0) {
                    1 => ModelKind::Cloze,
                    _ => ModelKind::Basic,
                };
                models.insert(id, kind);
            }
        }
    }
    Ok(models)
}

async fn load_cards(pool: &SqlitePool) -> Result<Vec<CardRecord>> {
    let rows = sqlx::query(
        r#"
        SELECT
            cards.did  AS did,  -- deck id
            cards.ord  AS ord,  -- card order (template ordinal)
            notes.mid  AS mid,  -- model (note type) id
            notes.flds AS flds  -- packed field values
        FROM cards
        JOIN notes ON notes.id = cards.nid
        ORDER BY cards.did, notes.id, cards.ord
        "#,
    )
    .fetch_all(pool)
    .await?;
    let mut cards = Vec::with_capacity(rows.len());
    for row in rows {
        let deck_id: i64 = row.try_get("did")?;
        let card_order: i64 = row.try_get("ord")?;
        let model_id: i64 = row.try_get("mid")?;

        //"Examples of supervised methods with built-in feature selection\u{1f}Decision trees<br><div>LASSO (linear regression with L1 regularization)</div>\u{1f}<a href=\"https://machinelearningmastery.com/feature-selection-with-real-and-categorical-data/\">https://machinelearningmastery.com/feature-selection-with-real-and-categorical-data/</a>\u{1f}"
        let fields_raw: String = row.try_get("flds")?;
        let card = CardRecord {
            deck_id,
            model_id,
            card_order,
            fields: split_fields(&fields_raw),
        };
        cards.push(card);
    }
    println!(
        "Found {} cards in DB",
        Palette::paint(Palette::WARNING, cards.len())
    );
    Ok(cards)
}

fn build_exports(
    cards: Vec<CardRecord>,
    models: &HashMap<i64, ModelKind>,
) -> HashMap<i64, Vec<String>> {
    let mut per_deck: HashMap<i64, Vec<String>> = HashMap::new();
    let mut num_duplicates = 0;
    let mut content_hashes: HashSet<String> = HashSet::new();

    for card in cards {
        let Some(model) = models.get(&card.model_id) else {
            println!(
                "Card with an unknown model id found: {}",
                Palette::paint(Palette::DANGER, card.model_id)
            );
            continue;
        };
        let entry = match model {
            ModelKind::Basic => basic_entry(&card.fields, card.card_order),
            ModelKind::Cloze => cloze_entry(&card.fields),
        };

        let Some(content) = entry else {
            continue;
        };
        let Some(content_hash) = get_hash(&content) else {
            continue;
        };
        if !content_hashes.insert(content_hash) {
            num_duplicates += 1;
            continue;
        }
        per_deck.entry(card.deck_id).or_default().push(content);
    }
    println!(
        "Found {} duplicates",
        Palette::paint(Palette::WARNING, num_duplicates)
    );
    per_deck
}

fn write_exports(
    export_path: &Path,
    decks: &HashMap<i64, DeckInfo>,
    exports: HashMap<i64, Vec<String>>,
) -> Result<()> {
    for deck_id in decks.keys() {
        let exports_per_deck = exports.get(deck_id).map(|v| v.len()).unwrap_or(0);
        println!(
            "Deck {} has {} cards",
            Palette::paint(Palette::ACCENT, decks.get(deck_id).unwrap().name.as_str()),
            Palette::paint(Palette::WARNING, exports_per_deck)
        );
    }
    let mut entries: Vec<(i64, Vec<String>)> = exports
        .into_iter()
        .filter(|(_, cards)| !cards.is_empty())
        .collect();
    println!(
        "There are {} decks with at least one card",
        Palette::paint(Palette::WARNING, entries.len())
    );
    entries.sort_by(|(a, _), (b, _)| {
        let name_a = decks.get(a).map(|d| d.name.as_str()).unwrap_or("");
        let name_b = decks.get(b).map(|d| d.name.as_str()).unwrap_or("");
        name_a.cmp(name_b)
    });
    for (deck_id, cards) in entries {
        let deck = decks
            .get(&deck_id)
            .ok_or_else(|| anyhow!("missing deck metadata for id {}", deck_id))?;
        let mut path = PathBuf::from(export_path);
        if deck.components.len() > 1 {
            for component in &deck.components[..deck.components.len() - 1] {
                path.push(component);
            }
        }
        let file_stem = deck
            .components
            .last()
            .cloned()
            .unwrap_or_else(|| "Deck".to_string());
        path.push(format!("{file_stem}.md"));
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut content = String::new();
        for card in &cards {
            content.push_str(card);
        }
        println!(
            "Writing {} cards to {}",
            Palette::paint(Palette::WARNING, cards.len()),
            Palette::paint(Palette::ACCENT, path.display())
        );
        fs::write(&path, content)?;
    }
    Ok(())
}

fn split_fields(raw: &str) -> Vec<String> {
    raw.split('\x1f').map(clean_field).collect()
}

fn clean_field(field: &str) -> String {
    let mut text = field.replace("\r\n", "\n");
    text = text.replace("<br />", "\n");
    text = text.replace("<br>", "\n");
    text = text.replace("<div>", "\n");
    text = text.replace("</div>", "\n");
    text = text.replace("<p>", "\n");
    text = text.replace("</p>", "\n");
    text = text.replace("<li>", "\n- ");
    text = text.replace("</li>", "");
    let without_tags = TAG_RE.replace_all(&text, "");
    decode_html_entities(without_tags.trim()).to_string()
}

fn deck_components(name: &str) -> Vec<String> {
    let mut parts: Vec<String> = name
        .split("::")
        .map(sanitize_component)
        .filter(|part| !part.is_empty())
        .collect();
    if parts.is_empty() {
        parts.push("Deck".to_string());
    }
    parts
}

fn sanitize_component(input: &str) -> String {
    let trimmed = input.trim().trim_start_matches('.');
    if trimmed.is_empty() {
        return String::new();
    }
    let mut out = String::with_capacity(trimmed.len());
    for ch in trimmed.chars() {
        if matches!(ch, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
            out.push('-');
        } else {
            out.push(ch);
        }
    }
    out.trim().to_string()
}

fn basic_entry(fields: &[String], ord: i64) -> Option<String> {
    if fields.len() < 2 {
        return None;
    }
    let (question, answer) = if ord % 2 == 0 {
        (&fields[0], &fields[1])
    } else {
        (&fields[1], &fields[0])
    };
    let mut entry = format_section("Q", question)?;
    entry.push_str(&format_section("A", answer)?);
    entry.push('\n');
    Some(entry)
}

fn cloze_entry(fields: &[String]) -> Option<String> {
    let text = fields.first()?;
    let converted = convert_cloze(text);
    let mut entry = format_section("C", converted.trim())?;
    entry.push('\n');
    Some(entry)
}

fn format_section(label: &str, value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    let mut out = String::new();
    out.push_str(label);
    out.push_str(": ");
    out.push_str(trimmed);
    out.push('\n');
    Some(out)
}

fn convert_cloze(text: &str) -> String {
    CLOZE_RE
        .replace_all(text, |caps: &regex::Captures| {
            let inner = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            format!("[{}]", inner.trim())
        })
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_field_strips_markup_and_decodes_entities() {
        let input = "<div>Hello &amp; <strong>world</strong></div>";
        assert_eq!(clean_field(input), "Hello & world");
    }

    #[test]
    fn basic_entry_swaps_fields_on_reverse_cards() {
        let fields = vec!["Front".into(), "Back".into()];
        let forward = basic_entry(&fields, 0).unwrap();
        assert!(forward.contains("Q: Front"));
        assert!(forward.contains("A: Back"));

        let reverse = basic_entry(&fields, 1).unwrap();
        assert!(reverse.contains("Q: Back"));
        assert!(reverse.contains("A: Front"));

        assert!(basic_entry(&["Only".into()], 0).is_none());
    }

    #[test]
    fn convert_cloze_rewrites_all_cloze_blocks() {
        let text = "Capital {{c1::Tokyo}} and {{c2::Kyoto::hint}}";
        assert_eq!(convert_cloze(text), "Capital [Tokyo] and [Kyoto]");
    }

    #[test]
    fn deck_components_sanitizes_segments_and_falls_back() {
        assert_eq!(
            deck_components("Data Science::/ETL?:"),
            vec!["Data Science".to_string(), "-ETL--".to_string()]
        );
        assert_eq!(deck_components(""), vec!["Deck".to_string()]);
    }
}
