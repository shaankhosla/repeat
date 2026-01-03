use std::io::{self, Write};

use anyhow::{Context, Result, anyhow};
use async_openai::types::{
    ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs,
    CreateChatCompletionRequestArgs,
};
use async_openai::{Client, config::OpenAIConfig};

const SERVICE: &str = "com.repeat.cli";
const USERNAME: &str = "openai";

use keyring::Entry;
use tokio::runtime::Builder;

const CLOZE_MODEL: &str = "gpt-4o-mini";

pub fn generate_cloze(text: &str) -> Vec<String> {
    if text.trim().is_empty() {
        return Vec::new();
    }

    let user_prompt = get_cloze_prompt(text);
    let text = text.to_string();
    let runtime = match Builder::new_current_thread().enable_all().build() {
        Ok(rt) => rt,
        Err(err) => {
            eprintln!(
                "repeat: unable to start runtime for LLM cloze generation: {}",
                err
            );
            return Vec::new();
        }
    };

    runtime.block_on(async move {
        match llm_status(&user_prompt).await {
            Ok(client) => match request_cloze(&client, &text).await {
                Ok(suggestions) => suggestions,
                Err(err) => {
                    eprintln!("repeat: failed to generate cloze: {}", err);
                    Vec::new()
                }
            },
            Err(err) => {
                eprintln!("repeat: LLM unavailable: {}", err);
                Vec::new()
            }
        }
    })
}

fn get_cloze_prompt(text: &str) -> String {
    let mut prompt = String::new();
    prompt.push_str(
        "There is a Cloze text in your collection that doesn't have a valid cloze []:\n\n",
    );
    prompt.push_str(text);
    prompt.push_str("\nIf you'd like to use an LLM to turn this into a Cloze, we can send your text to an LLM to generate a Cloze.\n\n");
    prompt
}

async fn llm_status(user_prompt: &str) -> Result<Client<OpenAIConfig>> {
    let llm_key = load_api_key();
    let key = match llm_key {
        Ok(api_key) => api_key,
        Err(_) => {
            let api_key = prompt_user_for_key(user_prompt)?;
            if api_key.is_empty() {
                return Err(anyhow!(
                    "No OpenAI API key provided; cannot generate Cloze text automatically"
                ));
            }
            store_api_key(&api_key)?;
            api_key
        }
    };
    let client = initialize_client(&key).await?;
    healthcheck_client(&client).await?;
    Ok(client)
}

async fn initialize_client(api_key: &str) -> Result<Client<OpenAIConfig>> {
    let config = OpenAIConfig::new()
        .with_api_key(api_key)
        .with_org_id("the-continental");

    let client = Client::with_config(config);
    Ok(client)
}

async fn healthcheck_client(client: &Client<OpenAIConfig>) -> Result<()> {
    client
        .models()
        .list()
        .await
        .context("Failed to list models")?;
    Ok(())
}

async fn request_cloze(client: &Client<OpenAIConfig>, text: &str) -> Result<Vec<String>> {
    let request = CreateChatCompletionRequestArgs::default()
        .model(CLOZE_MODEL)
        .max_tokens(200_u16)
        .temperature(0.2)
        .messages([
            ChatCompletionRequestSystemMessageArgs::default()
                .content("You rewrite study notes into Cloze deletions. Wrap the hidden facts in square brackets []. Return up to three options, one per line, and do not add commentary.")
                .build()?
                .into(),
            ChatCompletionRequestUserMessageArgs::default()
                .content(format!(
                    "Turn the following text into a Cloze card by inserting [] around the hidden portion:\n{}",
                    text
                ))
                .build()?
                .into(),
        ])
        .build()?;

    let response = client
        .chat()
        .create(request)
        .await
        .context("Failed to request Cloze generation")?;

    let mut suggestions = Vec::new();
    for choice in response.choices {
        if let Some(content) = choice.message.content {
            for line in content.lines() {
                let candidate = line
                    .trim()
                    .trim_start_matches(|c: char| c == '-' || c == '*')
                    .trim();
                if !candidate.is_empty() {
                    suggestions.push(candidate.to_string());
                }
            }
        }
    }
    Ok(suggestions)
}

fn prompt_user_for_key(prompt: &str) -> Result<String> {
    // let dim = "\x1b[2m";
    let reset = "\x1b[0m";
    // let cyan = "\x1b[36m";
    // let red = "\x1b[31m";
    let green = "\x1b[32m";
    // let blue = "\x1b[34m";

    println!("{}", prompt);
    println!(
        "{green}If you'd like to use an LLM to turn this into a Cloze, enter your OpenAI API{reset} key (https://platform.openai.com/account/api-keys) if you'd like to use this feature. It's stored locally for future use. Leave blank if not."
    );
    let _ = io::stdout().flush();

    let mut user_input = String::new();
    io::stdin().read_line(&mut user_input)?;
    let trimmed = user_input.trim();
    Ok(trimmed.to_string())
}

fn store_api_key(api_key: &str) -> Result<()> {
    let entry = Entry::new(SERVICE, USERNAME)?;
    entry.set_password(api_key)?;
    Ok(())
}

fn load_api_key() -> Result<String> {
    let entry = Entry::new(SERVICE, USERNAME)?;
    Ok(entry.get_password()?)
}
