use anyhow::{Context, Result, anyhow, bail};
use openai::{
    Credentials, DEFAULT_BASE_URL,
    chat::{
        ChatCompletion, ChatCompletionMessage, ChatCompletionMessageRole,
        ChatCompletionResponseFormat,
    },
};
use std::{ops::Deref, sync::Arc};
use tiktoken_rs::{CoreBPE, p50k_base};

const ANALYSIS_MAX_TOKENS: u64 = 1500;
const INPUT_TOKEN_LIMIT: u64 = 270000; // 2000 less than max for gpt5.2

pub struct AnalyzerInner {
    model: String,
    credentials: Credentials,
    system_prompt: String,
    tokenizer: CoreBPE,
}

#[derive(Clone)]
pub struct Analyzer {
    inner: Arc<AnalyzerInner>,
}

/// Returns the longest prefix of `text` whose token count
/// is <= INPUT_TOKEN_LIMIT.
///
/// Guarantees:
/// - Output is always a prefix of `text`
/// - No Unicode / emoji / grapheme corruption
/// - Deterministic
pub fn respect_token_limit(text: &str, tokenizer: &CoreBPE) -> String {
    // Fast path
    if tokenizer.encode_with_special_tokens(text).len() as u64 <= INPUT_TOKEN_LIMIT {
        return text.to_owned();
    }

    let mut low = 0;
    let mut high = text.len(); // byte index

    // Binary search over byte indices that are valid UTF-8 boundaries
    while low < high {
        let mid = (low + high + 1) / 2;

        // Ensure mid is on a UTF-8 char boundary
        let mid = match text.is_char_boundary(mid) {
            true => mid,
            false => {
                // walk backward to nearest valid boundary
                (0..mid).rev().find(|&i| text.is_char_boundary(i)).unwrap()
            }
        };

        let prefix = &text[..mid];
        let token_len = tokenizer.encode_with_special_tokens(prefix).len() as u64;

        if token_len <= INPUT_TOKEN_LIMIT {
            low = mid;
        } else {
            high = mid - 1;
        }
    }

    // Final slice must also be on a char boundary
    let final_idx = if text.is_char_boundary(low) {
        low
    } else {
        (0..low).rev().find(|&i| text.is_char_boundary(i)).unwrap()
    };

    text[..final_idx].to_owned()
}

impl Deref for Analyzer {
    type Target = AnalyzerInner;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl Analyzer {
    pub fn new(
        system_prompt: String,
        model: String,
        api_key: String,
        base_url: Option<String>,
    ) -> Self {
        let credentials =
            Credentials::new(api_key, base_url.unwrap_or(DEFAULT_BASE_URL.to_string()));
        let inner = Arc::new(AnalyzerInner {
            system_prompt,
            model,
            credentials,
            tokenizer: p50k_base().expect("Failed to load p50k_base tokenizer"),
        });
        Self { inner }
    }

    pub async fn analyze<T>(&self, input_json: String) -> Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        let input_json = respect_token_limit(&input_json, &self.tokenizer);
        let input = vec![
            ChatCompletionMessage {
                role: ChatCompletionMessageRole::System,
                content: Some(self.system_prompt.clone()),
                ..Default::default()
            },
            ChatCompletionMessage {
                role: ChatCompletionMessageRole::User,
                content: Some(input_json),
                ..Default::default()
            },
        ];
        let chat_completion = ChatCompletion::builder(&self.model, input)
            .credentials(self.credentials.clone())
            .response_format(ChatCompletionResponseFormat::json_object())
            .max_completion_tokens(ANALYSIS_MAX_TOKENS)
            .temperature(0.1)
            .create()
            .await
            .context("Failed to create analysis")?;
        let choice = chat_completion
            .choices
            .first()
            .ok_or_else(|| anyhow!("No choices returned from model"))?;
        let returned_message = choice.message.clone();
        let content = returned_message
            .content
            .ok_or_else(|| anyhow!("Model failed to generate valid analysis"))?
            .trim()
            .to_string();
        if content.is_empty() {
            bail!("Model returned empty analysis");
        }
        if choice.finish_reason != "stop" {
            bail!(
                "Model response was incomplete (finish_reason={})",
                choice.finish_reason
            );
        }
        serde_json::from_str(&content).context("Failed to parse model analysis response JSON")
    }
}
