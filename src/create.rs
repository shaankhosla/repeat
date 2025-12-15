use crate::utils::validate_file;

use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::Result;
use crossterm::{
    event::{
        self, Event, KeyCode, KeyEventKind, KeyModifiers, KeyboardEnhancementFlags,
        PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::Stylize,
    widgets::{Block, Borders, Paragraph, Wrap},
};

pub fn run(card_path: String) -> Result<()> {
    let card_path = validate_file(card_path)?;
    process_card(card_path)
}

fn process_card(card_path: PathBuf) -> Result<()> {
    let card_exists = card_path.is_file();
    if !card_exists {
        if !prompt_create(&card_path)? {
            println!("Aborting; card not created.");
            return Ok(());
        }
        if let Some(parent) = card_path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent)?;
        }
    }

    capture_cards(&card_path)?;
    Ok(())
}

fn prompt_create(path: &Path) -> io::Result<bool> {
    print!(
        "Card '{}' does not exist. Create it? [y/N]: ",
        path.display()
    );
    io::stdout().flush()?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    let trimmed = answer.trim().to_lowercase();
    Ok(trimmed == "y" || trimmed == "yes")
}

fn append_to_card(path: &Path, contents: &str) -> io::Result<()> {
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    let trimmed = contents.trim_end_matches('\n');
    if trimmed.is_empty() {
        return Ok(());
    }

    let has_existing_content = file.metadata()?.len() > 0;
    if has_existing_content {
        writeln!(file)?;
    }
    writeln!(file, "{}", trimmed)?;
    Ok(())
}

fn capture_cards(card_path: &Path) -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        EnterAlternateScreen,
        PushKeyboardEnhancementFlags(
            KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
                | KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
        )
    )?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.show_cursor()?;

    let editor_result: io::Result<()> = (|| {
        let mut input = String::new();
        let mut status: Option<String> = None;
        loop {
            terminal.draw(|frame| {
                let area = frame.area();
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(3), Constraint::Length(4)])
                    .split(area);

                let editor_block = Block::default()
                    .title(format!(" {} ", card_path.display()).bold())
                    .borders(Borders::ALL);
                let editor = Paragraph::new(input.as_str())
                    .block(editor_block)
                    .wrap(Wrap { trim: false });
                frame.render_widget(editor, chunks[0]);

                let mut help =
                    String::from("Ctrl+S to save • Esc/Ctrl-C to exit • Enter for newline");
                if let Some(message) = &status {
                    help.push('\n');
                    help.push_str(message);
                }
                let instructions = Paragraph::new(help)
                    .block(Block::default().borders(Borders::ALL).title(" Help "));
                frame.render_widget(instructions, chunks[1]);

                let cursor_line = input.split('\n').count().saturating_sub(1) as u16;
                let last_line = input.rsplit('\n').next().unwrap_or("");
                let cursor_col = last_line.chars().count() as u16;

                let cursor_x = chunks[0].x + 1 + cursor_col.min(chunks[0].width.saturating_sub(2));
                let cursor_y =
                    chunks[0].y + 1 + cursor_line.min(chunks[0].height.saturating_sub(2));
                frame.set_cursor_position((cursor_x, cursor_y));
            })?;

            if event::poll(Duration::from_millis(250))?
                && let Event::Key(key) = event::read()?
            {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                if key.code == KeyCode::Esc
                    || (key.code == KeyCode::Char('c')
                        && key.modifiers.contains(KeyModifiers::CONTROL))
                {
                    break;
                }

                if key.code == KeyCode::Char('s') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    if input.trim().is_empty() {
                        status = Some(String::from("Card not saved (empty)."));
                    } else {
                        append_to_card(card_path, &input)?;
                        status = Some(String::from("Card saved."));
                    }
                    input.clear();
                    continue;
                }

                match key.code {
                    KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                        input.push(c);
                    }
                    KeyCode::Enter => input.push('\n'),
                    KeyCode::Tab => input.push('\t'),
                    KeyCode::Backspace => {
                        input.pop();
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    })();

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        PopKeyboardEnhancementFlags,
        LeaveAlternateScreen
    )?;
    terminal.show_cursor()?;

    editor_result
}
