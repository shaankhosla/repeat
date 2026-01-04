use std::io::{self, Write};

use anyhow::{Context, Result, anyhow};
use async_openai::types::{
    ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs,
    CreateChatCompletionRequestArgs,
};
use async_openai::{Client, config::OpenAIConfig};

const SERVICE: &str = "com.repeat.cl";
const USERNAME: &str = "openai";

use keyring::Entry;

const CLOZE_MODEL: &str = "gpt-5-nano";
const SYSTEM_PROMPT: &str = r#"
You convert flashcards into Cloze deletions.
A Cloze deletion is denoted by square brackets: [hidden text].
Only add one Cloze deletion.
"#;

const USER_PROMPT_TEMPLATE: &str = r#"
Turn the following text into a Cloze card by inserting [] around the hidden portion.
Return the exact same text, only with brackets added.

Text:
{}
"#;

pub async fn ensure_client(user_prompt: &str) -> Result<Client<OpenAIConfig>> {
    let llm_key = load_api_key();
    let key = match llm_key {
        Ok(api_key) => api_key,
        Err(_) => {
            let api_key = prompt_user_for_key(user_prompt)?;
            if api_key.is_empty() {
                return Err(anyhow!("No API key provided"));
            }
            store_api_key(&api_key)?;
            api_key
        }
    };
    let client = initialize_client(&key)?;
    healthcheck_client(&client).await?;
    Ok(client)
}

fn initialize_client(api_key: &str) -> Result<Client<OpenAIConfig>> {
    let config = OpenAIConfig::new().with_api_key(api_key);

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

pub async fn request_cloze(client: &Client<OpenAIConfig>, text: &str) -> Result<String> {
    let request = CreateChatCompletionRequestArgs::default()
        .model(CLOZE_MODEL)
        .max_tokens(200)
        .temperature(0.2)
        .messages([
            ChatCompletionRequestSystemMessageArgs::default()
                .content(SYSTEM_PROMPT)
                .build()?
                .into(),
            ChatCompletionRequestUserMessageArgs::default()
                .content(format!(USER_PROMPT_TEMPLATE, text))
                .build()?
                .into(),
        ])
        .build()?;

    let response = client.chat().create(request).await?;

    let output = response
        .choices
        .first()
        .and_then(|c| c.message.content.clone())
        .ok_or_else(|| anyhow!("No content returned from model"))?;

    Ok(output)
}

fn prompt_user_for_key(prompt: &str) -> Result<String> {
    let dim = "\x1b[2m";
    let reset = "\x1b[0m";
    let green = "\x1b[32m";

    println!("\n{}", prompt);
    println!(
        "{green}Enter your OpenAI API key{reset} (https://platform.openai.com/account/api-keys) to enable the LLM helper. It's stored locally for future use.",
        green = green,
        reset = reset
    );
    println!(
        "{dim}Leave the field blank to skip—repeat will continue without sending anything.{reset}",
        dim = dim,
        reset = reset
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
