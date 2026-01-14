use anyhow::{Result, bail};
use async_openai::{
    Client,
    config::OpenAIConfig,
    types::responses::{
        CreateResponseArgs, InputMessage, InputRole, OutputItem, OutputMessageContent,
    },
};

const REPHRASE_MODEL: &str = "gpt-5-nano";

const SYSTEM_PROMPT: &str = r#"
You rewrite flashcard questions to be clearer while keeping the same fact and difficulty.
Never reveal the answer inside the question and keep the tone neutral.
"#;

pub async fn request_question_rephrase(
    client: &Client<OpenAIConfig>,
    question: &str,
    answer: &str,
) -> Result<String> {
    let user_prompt = format!(
        "Rewrite the question below so it is clearer, but keep the meaning the same.\n\
         Return only the rewritten question.\n\n\
         Question: {question}\n\
         Answer (for context; do not reveal): {answer}"
    );

    let request = CreateResponseArgs::default()
        .model(REPHRASE_MODEL)
        .max_output_tokens(5000_u32)
        .input(vec![
            InputMessage {
                role: InputRole::System,
                content: vec![SYSTEM_PROMPT.into()],
                status: None,
            },
            InputMessage {
                role: InputRole::User,
                content: vec![user_prompt.into()],
                status: None,
            },
        ])
        .build()?;

    let response = client.responses().create(request).await?;

    for item in response.output {
        if let OutputItem::Message(message) = item {
            for content in message.content {
                if let OutputMessageContent::OutputText(text) = content {
                    let trimmed = text.text.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    return Ok(trimmed.to_string());
                }
            }
        }
    }

    bail!("No text output returned from model")
}
