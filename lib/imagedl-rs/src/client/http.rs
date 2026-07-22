//! HTTP client wrapper with retry logic, header management, and proxy support.
//!
//! Wraps `reqwest::Client` which internally maintains a connection pool
//! (equivalent to Python's `requests.Session`).

use std::time::Duration;
use reqwest::header::HeaderMap;
use crate::error::{ImageDlError, Result};

/// HTTP client with retry, proxy, and header management.
#[derive(Clone)]
pub struct HttpClient {
    client: reqwest::Client,
    max_retries: usize,
    default_headers: HeaderMap,
}

impl HttpClient {
    /// Create a new HTTP client from a builder configuration.
    pub(crate) fn from_builder(builder: HttpClientBuilder) -> Result<Self> {
        let mut client_builder = reqwest::Client::builder()
            .cookie_store(true)
            .gzip(true)
            .brotli(true)
            .timeout(builder.timeout);

        if let Some(proxy) = &builder.proxy {
            let reqwest_proxy = reqwest::Proxy::all(proxy)
                .map_err(|e| ImageDlError::Other(format!("Invalid proxy URL: {}", e)))?;
            client_builder = client_builder.proxy(reqwest_proxy);
        }

        let client = client_builder.build()
            .map_err(|e| ImageDlError::Other(format!("Failed to build HTTP client: {}", e)))?;

        Ok(Self {
            client,
            max_retries: builder.max_retries,
            default_headers: builder.default_headers,
        })
    }

    /// GET request returning the response body as text.
    ///
    /// Retries on failure up to `max_retries` times.
    pub async fn get_text(&self, url: &str, headers: HeaderMap) -> Result<String> {
        let mut last_err = None;
        for attempt in 0..=self.max_retries {
            match self.try_get_text(url, &headers).await {
                Ok(text) => return Ok(text),
                Err(e) => {
                    if attempt < self.max_retries {
                        log::warn!("GET {} attempt {} failed: {}", url, attempt + 1, e);
                    }
                    last_err = Some(e);
                }
            }
        }
        Err(last_err.unwrap())
    }

    /// GET request returning the response body as bytes.
    ///
    /// Retries on failure up to `max_retries` times.
    pub async fn get_bytes(&self, url: &str, headers: HeaderMap) -> Result<bytes::Bytes> {
        let mut last_err = None;
        for attempt in 0..=self.max_retries {
            match self.try_get_bytes(url, &headers).await {
                Ok(bytes) => return Ok(bytes),
                Err(e) => {
                    if attempt < self.max_retries {
                        log::warn!("GET {} attempt {} failed: {}", url, attempt + 1, e);
                    }
                    last_err = Some(e);
                }
            }
        }
        Err(last_err.unwrap())
    }

    /// POST request returning the response body as text.
    ///
    /// Retries on failure up to `max_retries` times.
    pub async fn post_text(
        &self,
        url: &str,
        body: &str,
        headers: HeaderMap,
    ) -> Result<String> {
        let mut last_err = None;
        for attempt in 0..=self.max_retries {
            match self.try_post_text(url, body, &headers).await {
                Ok(text) => return Ok(text),
                Err(e) => {
                    if attempt < self.max_retries {
                        log::warn!("POST {} attempt {} failed: {}", url, attempt + 1, e);
                    }
                    last_err = Some(e);
                }
            }
        }
        Err(last_err.unwrap())
    }

    /// Get the default headers.
    pub fn default_headers(&self) -> &HeaderMap {
        &self.default_headers
    }

    async fn try_get_text(&self, url: &str, headers: &HeaderMap) -> Result<String> {
        let merged = self.merge_headers(headers);
        let resp = self.client.get(url).headers(merged).send().await?;
        resp.error_for_status_ref()?;
        Ok(resp.text().await?)
    }

    async fn try_get_bytes(&self, url: &str, headers: &HeaderMap) -> Result<bytes::Bytes> {
        let merged = self.merge_headers(headers);
        let resp = self.client.get(url).headers(merged).send().await?;
        resp.error_for_status_ref()?;
        Ok(resp.bytes().await?)
    }

    async fn try_post_text(
        &self,
        url: &str,
        body: &str,
        headers: &HeaderMap,
    ) -> Result<String> {
        let merged = self.merge_headers(headers);
        let resp = self.client
            .post(url)
            .headers(merged)
            .body(body.to_string())
            .send()
            .await?;
        resp.error_for_status_ref()?;
        Ok(resp.text().await?)
    }

    /// Merge request-specific headers with default headers.
    /// Request-specific headers take precedence.
    fn merge_headers(&self, request_headers: &HeaderMap) -> HeaderMap {
        let mut merged = self.default_headers.clone();
        for (name, value) in request_headers.iter() {
            merged.insert(name.clone(), value.clone());
        }
        merged
    }
}

/// Builder for `HttpClient`.
pub struct HttpClientBuilder {
    max_retries: usize,
    timeout: Duration,
    proxy: Option<String>,
    default_headers: HeaderMap,
}

impl HttpClientBuilder {
    /// Create a new builder with sensible defaults.
    pub fn new() -> Self {
        Self {
            max_retries: 5,
            timeout: Duration::from_secs(30),
            proxy: None,
            default_headers: HeaderMap::new(),
        }
    }

    /// Set the maximum number of retries for failed requests.
    pub fn max_retries(mut self, n: usize) -> Self {
        self.max_retries = n;
        self
    }

    /// Set the request timeout.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set a proxy URL for all requests.
    pub fn proxy(mut self, proxy_url: impl Into<String>) -> Self {
        self.proxy = Some(proxy_url.into());
        self
    }

    /// Set default headers that will be included in every request.
    pub fn default_headers(mut self, headers: HeaderMap) -> Self {
        self.default_headers = headers;
        self
    }

    /// Build the `HttpClient`.
    pub fn build(self) -> Result<HttpClient> {
        HttpClient::from_builder(self)
    }
}

impl Default for HttpClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}
