# Card Format

Store decks anywhere, for example:

```
flashcards/
  math.md
  science/
      physics.md
      chemistry.md
```

Cards live in everyday Markdown. `repeat` scans for tagged sections and turns them into flashcards, so you can mix active-recall prompts with your normal notes.

- **Basic cards**

  ```markdown
  Q: What is Coulomb's constant?
  A: The proportionality constant of the electric force.
  ```

- **Cloze cards**

  ```markdown
  C: The [order] of a group is [the cardinality of its underlying set].
  ```

## Parsing Logic

- Cards are detected by the presence of a `Q:/A:` or `C:` block. A horizontal rule (`---`) or the start of another card marks the end.
- Cards are hashed with Blake3; editing the text resets the card's performance history.
- Metadata lives in `cards.db` under your OS data directory (for example, `~/Library/Application Support/repeat/cards.db` on macOS). Delete this file to reset history; the Markdown decks remain untouched.
- Multi-line content is supported.
