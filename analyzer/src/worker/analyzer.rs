use anyhow::{Context, Result, anyhow, bail};
use openai::{
    Credentials, DEFAULT_BASE_URL,
    chat::{
        ChatCompletion, ChatCompletionMessage, ChatCompletionMessageRole,
        ChatCompletionResponseFormat,
    },
};
use std::{ops::Deref, sync::Arc};

const ANALYSIS_MAX_TOKENS: u64 = 1500;

pub struct AnalyzerInner {
    model: String,
    credentials: Credentials,
    system_prompt: String,
}

#[derive(Clone)]
pub struct Analyzer {
    inner: Arc<AnalyzerInner>,
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
        });
        Self { inner }
    }

    pub async fn analyze<T>(&self, input_json: String) -> Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
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
