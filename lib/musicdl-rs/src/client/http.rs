//! HTTP client wrapper with retry logic, header management, and proxy support.
//!
//! Wraps `reqwest::Client` which internally maintains a connection pool
//! (equivalent to Python's `requests.Session`). Extends imagedl-rs's HttpClient
//! with JSON parsing and audio link testing methods for music sources.

use reqwest::header::{CONTENT_TYPE, HeaderMap, RANGE};
use serde::de::DeserializeOwned;
use std::time::Duration;

use crate::{
    detect::{AudioFormatDetector, is_valid_audio_ext},
    error::{MusicDlError, Result},
    types::DownloadUrlStatus,
};

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
                .map_err(|e| MusicDlError::Other(format!("Invalid proxy URL: {}", e)))?;
            client_builder = client_builder.proxy(reqwest_proxy);
        }

        let client = client_builder
            .build()
            .map_err(|e| MusicDlError::Other(format!("Failed to build HTTP client: {}", e)))?;

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

    /// GET request returning parsed JSON.
    ///
    /// Retries on failure up to `max_retries` times.
    pub async fn get_json<T: DeserializeOwned>(&self, url: &str, headers: HeaderMap) -> Result<T> {
        let mut last_err = None;
        for attempt in 0..=self.max_retries {
            match self.try_get_json(url, &headers).await {
                Ok(data) => return Ok(data),
                Err(e) => {
                    if attempt < self.max_retries {
                        log::warn!("GET JSON {} attempt {} failed: {}", url, attempt + 1, e);
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
    pub async fn post_text(&self, url: &str, body: &str, headers: HeaderMap) -> Result<String> {
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

    /// POST request with JSON body, returning parsed JSON.
    ///
    /// Retries on failure up to `max_retries` times.
    pub async fn post_json<T: DeserializeOwned>(
        &self,
        url: &str,
        json: &serde_json::Value,
        headers: HeaderMap,
    ) -> Result<T> {
        let mut last_err = None;
        for attempt in 0..=self.max_retries {
            match self.try_post_json(url, json, &headers).await {
                Ok(data) => return Ok(data),
                Err(e) => {
                    if attempt < self.max_retries {
                        log::warn!("POST JSON {} attempt {} failed: {}", url, attempt + 1, e);
                    }
                    last_err = Some(e);
                }
            }
        }
        Err(last_err.unwrap())
    }

    /// POST request with form data, returning parsed JSON.
    pub async fn post_form_json<T: DeserializeOwned>(
        &self,
        url: &str,
        form: &[(&str, &str)],
        headers: HeaderMap,
    ) -> Result<T> {
        let mut last_err = None;
        for attempt in 0..=self.max_retries {
            match self.try_post_form_json(url, form, &headers).await {
                Ok(data) => return Ok(data),
                Err(e) => {
                    if attempt < self.max_retries {
                        log::warn!("POST FORM {} attempt {} failed: {}", url, attempt + 1, e);
                    }
                    last_err = Some(e);
                }
            }
        }
        Err(last_err.unwrap())
    }

    /// GET request returning raw Response for streaming downloads.
    pub async fn get_streaming(&self, url: &str, headers: HeaderMap) -> Result<reqwest::Response> {
        let merged = self.merge_headers(&headers);
        let resp = self.client.get(url).headers(merged).send().await?;
        resp.error_for_status_ref()?;
        Ok(resp)
    }

    /// Test an audio download URL: verify it's reachable, detect format and size.
    ///
    /// Mirrors Python's `AudioLinkTester.test()`. Makes a small-range GET request
    /// to check the URL returns valid audio data.
    pub async fn test_audio_link(
        &self,
        url: &str,
        headers: HeaderMap,
    ) -> Result<DownloadUrlStatus> {
        let mut test_headers = self.merge_headers(&headers);
        // Request only the first 1KB to test the link
        test_headers.insert(RANGE, "bytes=0-1023".parse().unwrap());

        let resp = match self.client.get(url).headers(test_headers).send().await {
            Ok(r) => r,
            Err(_e) => {
                return Ok(DownloadUrlStatus {
                    ok: false,
                    ext: None,
                    file_size_bytes: None,
                    file_size: None,
                    download_url: None,
                });
            }
        };

        let content_type = resp
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        // Try to get total size from Content-Range or Content-Length
        let file_size_bytes = resp
            .headers()
            .get("Content-Range")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.split('/').next_back())
            .and_then(|s| s.parse::<u64>().ok())
            .or_else(|| {
                resp.headers()
                    .get(reqwest::header::CONTENT_LENGTH)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok())
            });

        // Detect format from Content-Type or URL extension
        let ext = AudioFormatDetector::from_content_type(content_type)
            .map(|f| f.extension().to_string())
            .or_else(|| AudioFormatDetector::ext_from_url(url).map(|e| e.to_string()));

        // If format not detected from headers/URL, try from response body
        let ext = if ext.is_none() {
            match resp.bytes().await {
                Ok(b) => AudioFormatDetector::detect(&b).map(|f| f.extension().to_string()),
                Err(_) => None,
            }
        } else {
            ext
        };

        let is_valid_ext = ext.as_deref().map(is_valid_audio_ext).unwrap_or(false);

        Ok(DownloadUrlStatus {
            ok: is_valid_ext,
            ext,
            file_size_bytes,
            file_size: file_size_bytes.map(crate::utils::bytes_to_mb),
            download_url: Some(url.to_string()),
        })
    }

    /// Get the default headers.
    pub fn default_headers(&self) -> &HeaderMap {
        &self.default_headers
    }

    // --- Private implementation methods ---

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

    async fn try_get_json<T: DeserializeOwned>(&self, url: &str, headers: &HeaderMap) -> Result<T> {
        let merged = self.merge_headers(headers);
        let resp = self.client.get(url).headers(merged).send().await?;
        resp.error_for_status_ref()?;
        Ok(resp.json::<T>().await?)
    }

    async fn try_post_text(&self, url: &str, body: &str, headers: &HeaderMap) -> Result<String> {
        let merged = self.merge_headers(headers);
        let resp = self
            .client
            .post(url)
            .headers(merged)
            .body(body.to_string())
            .send()
            .await?;
        resp.error_for_status_ref()?;
        Ok(resp.text().await?)
    }

    async fn try_post_json<T: DeserializeOwned>(
        &self,
        url: &str,
        json: &serde_json::Value,
        headers: &HeaderMap,
    ) -> Result<T> {
        let merged = self.merge_headers(headers);
        let resp = self
            .client
            .post(url)
            .headers(merged)
            .json(json)
            .send()
            .await?;
        resp.error_for_status_ref()?;
        Ok(resp.json::<T>().await?)
    }

    async fn try_post_form_json<T: DeserializeOwned>(
        &self,
        url: &str,
        form: &[(&str, &str)],
        headers: &HeaderMap,
    ) -> Result<T> {
        let merged = self.merge_headers(headers);
        // Build form body manually since reqwest::form() requires multipart feature
        let form_encoded: String = form
            .iter()
            .map(|(k, v)| format!("{}={}", urlencoding::encode(k), urlencoding::encode(v)))
            .collect::<Vec<_>>()
            .join("&");
        let mut form_headers = merged;
        form_headers.insert(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded".parse().unwrap(),
        );
        let resp = self
            .client
            .post(url)
            .headers(form_headers)
            .body(form_encoded)
            .send()
            .await?;
        resp.error_for_status_ref()?;
        Ok(resp.json::<T>().await?)
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
            max_retries: 3,
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
