use crate::crud::{CardStats, DB};
use crate::drill::register_all_cards;

use anyhow::Result;

pub async fn run(db: &DB, paths: Vec<String>) -> Result<usize> {
    let card_hashes = register_all_cards(db, paths).await?;
    let count = card_hashes.len();
    let stats = db.collection_stats(&card_hashes).await?;
    print_stats(&stats);
    Ok(count)
}

fn print_stats(stats: &CardStats) {
    println!(
        "Number of cards {} • new {} • reviewed {}",
        stats.num_cards, stats.new_cards, stats.reviewed_cards
    );
    println!(
        "Due now: {} ({} overdue)",
        stats.due_cards, stats.overdue_cards
    );

    if !stats.upcoming_week.is_empty() {
        let total_due_next_week: i64 = stats.upcoming_week.iter().map(|b| b.count).sum();
        println!("Due in next 7 days: {}", total_due_next_week);
        for bucket in &stats.upcoming_week {
            println!("  {}: {}", bucket.day, bucket.count);
        }
    }
    println!("Due in next 30 days: {}", stats.upcoming_month);
    println!(
        "Total number of cards indexed in DB: {}",
        stats.total_cards_in_db
    );
}
