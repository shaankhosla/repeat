use crate::card::{Card, CardContent};
use crate::palette::Palette;

pub fn rephrase_user_prompt(cards: &[Card]) -> Option<String> {
    let mut count = 0usize;
    let mut sample_question: Option<String> = None;

    for card in cards {
        if let CardContent::Basic { question, .. } = &card.content {
            count += 1;
            if sample_question.is_none() {
                sample_question = Some(question.clone());
            }
        }
    }

    sample_question.map(|sample| build_user_prompt(count, &sample))
}

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
