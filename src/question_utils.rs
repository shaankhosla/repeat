use std::collections::HashMap;
use std::sync::Arc;

use crate::card::{Card, CardContent};
use crate::llm::{ensure_client, request_question_rephrase};
use crate::palette::Palette;

use anyhow::{Context, Result};
use async_openai::{Client, config::OpenAIConfig};
use futures::stream::{self, StreamExt};

const MAX_CONCURRENT_LLM_REQUESTS: usize = 4;

fn build_user_prompt(total: usize, sample_question: &str) -> String {
    let plural = if total == 1 { "" } else { "s" };
    format!(
        "\n{} can rephrase {} basic question{plural} before this drill session.\n\n{}\n{}\n",
        Palette::paint(Palette::INFO, "repeater"),
        Palette::paint(Palette::WARNING, total),
        Palette::dim("Example question:"),
        sample_question
    )
}

async fn replace_questions(
    cards: &mut [Card],
    cards_to_rephrase: Vec<(String, String, String)>,
    index_by_hash: &HashMap<String, usize>,
    client: Arc<Client<OpenAIConfig>>,
) -> Result<()> {
    let mut tasks = stream::iter(
        cards_to_rephrase
            .into_iter()
            .map(|(hash, question, answer)| {
                let client = Arc::clone(&client);
                async move {
                    let new_question = request_question_rephrase(&client, &question, &answer)
                        .await
                        .with_context(|| {
                            format!(
                                "Failed to rephrase question:\n\nQ: {}\nA: {}",
                                question, answer
                            )
                        })?;
                    Ok::<_, anyhow::Error>((hash, new_question))
                }
            }),
    )
    .buffer_unordered(MAX_CONCURRENT_LLM_REQUESTS);

    while let Some(result) = tasks.next().await {
        let (hash, rewritten) = result?;
        let Some(&idx) = index_by_hash.get(&hash) else {
            continue;
        };
        if let CardContent::Basic { question, .. } = &mut cards[idx].content {
            *question = rewritten;
        }
    }

    Ok(())
}

pub async fn rephrase_basic_questions(cards: &mut [Card]) -> Result<()> {
    let cards_to_rephrase: Vec<_> = cards
        .iter()
        .filter_map(|card| {
            if let CardContent::Basic { question, answer } = &card.content {
                Some((card.card_hash.clone(), question.clone(), answer.clone()))
            } else {
                None
            }
        })
        .collect();

    if cards_to_rephrase.is_empty() {
        return Ok(());
    }

    let user_prompt = build_user_prompt(cards_to_rephrase.len(), &cards_to_rephrase[0].1);
    let index_by_hash: HashMap<_, _> = cards
        .iter()
        .enumerate()
        .map(|(idx, card)| (card.card_hash.clone(), idx))
        .collect();

    let client = ensure_client(&user_prompt)
        .with_context(|| "Failed to initialize LLM client, cannot rephrase questions")?;
    let client = Arc::new(client);

    replace_questions(cards, cards_to_rephrase, &index_by_hash, client).await?;
    Ok(())
}
