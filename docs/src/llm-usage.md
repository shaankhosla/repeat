# LLM Usage

## LLM helper (opt-in)

## Opt in
- LLM calls are off until you provide an OpenAI API key.
- Skip every prompt to keep running fully offline.

## API keys
- `repeater llm --set <KEY>` saves the key via the OS keyring (`com.repeat/openai:default`), so macOS Keychain/Windows Credential Manager/libsecret hold it securely.
- `REPEATER_OPENAI_API_KEY` overrides the keyring for temporary runs.
- `repeater llm --test` confirms the key with OpenAI, `repeater llm --clear` forgets it instantly.

## Cloze generation
- Run `repeater drill <deck>`; if any `C:` cards lack `[]`, `repeater` sends that text to OpenAI (`gpt-5-nano`) and patches the file before the drill continues.
- Leave the API key prompt blank (or skip configuring a key) to keep the feature idle.

## Question rephrasing
- Run `repeater drill <deck> --rephrase` to rephrase basic `Q:` questions before the session starts.
- The original answers are provided as context but are not revealed in the rewritten questions.
