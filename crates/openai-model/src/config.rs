use std::fmt::Debug;

/// Builder for [`OpenAIConfig`].
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct OpenAIConfigBuilder {
    api_key: String,
    model: Option<String>,
    base_url: Option<String>,
}

impl OpenAIConfigBuilder {
    /// Creates a builder with the given API key.
    #[inline]
    pub fn with_api_key<S: Into<String>>(api_key: S) -> Self {
        Self {
            api_key: api_key.into(),
            model: None,
            base_url: None,
        }
    }

    /// Sets the model to use.
    #[inline]
    pub fn with_model<S: Into<String>>(mut self, model: S) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Sets a custom base URL.
    #[inline]
    pub fn with_base_url<S: Into<String>>(mut self, base_url: S) -> Self {
        self.base_url = Some(base_url.into());
        self
    }

    /// Builds the configuration.
    #[inline]
    pub fn build(self) -> OpenAIConfig {
        OpenAIConfig {
            api_key: self.api_key,
            model: self.model.unwrap_or_else(|| "gpt-5.2".to_string()),
            base_url: self
                .base_url
                .unwrap_or_else(|| "https://api.openai.com/v1".to_string()),
        }
    }
}

impl Debug for OpenAIConfigBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenAIConfigBuilder")
            .field("api_key", &"<deducted>")
            .field("model", &self.model)
            .field("base_url", &self.base_url)
            .finish()
    }
}

/// Configuration for the OpenAI-compatible provider.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct OpenAIConfig {
    pub(crate) api_key: String,
    pub(crate) model: String,
    pub(crate) base_url: String,
}

impl Debug for OpenAIConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenAIConfig")
            .field("api_key", &"<deducted>")
            .field("model", &self.model)
            .field("base_url", &self.base_url)
            .finish()
    }
}
