-- Create the cards table and supporting indexes.
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS cards (
    card_hash TEXT PRIMARY KEY,
    added_at TEXT NOT NULL,
    last_reviewed_at TEXT,
    stability REAL,
    difficulty REAL,
    interval_raw REAL,
    interval_days INTEGER,
    due_date TEXT,
    review_count INTEGER NOT NULL
) STRICT;

CREATE INDEX IF NOT EXISTS idx_cards_due_date ON cards(due_date);
