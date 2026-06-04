//! Feishu (Lark) API client for appending transcription text to documents.

use log::{debug, warn};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::time::Instant;

const BASE_URL: &str = "https://open.feishu.cn/open-apis";
const TOKEN_REFRESH_MARGIN_SECS: u64 = 300; // refresh 5min before expiry

pub struct FeishuClient {
    http: Client,
    app_id: String,
    app_secret: String,
    token_cache: Mutex<Option<CachedToken>>,
}

struct CachedToken {
    token: String,
    expires_at: Instant,
}

#[derive(Serialize)]
struct TokenRequest {
    app_id: String,
    app_secret: String,
}

#[derive(Deserialize)]
struct TokenResponse {
    code: i32,
    msg: String,
    tenant_access_token: Option<String>,
    expire: Option<u64>,
}

#[derive(Serialize)]
struct CreateDocumentRequest {
    title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    folder_token: Option<String>,
}

#[derive(Deserialize)]
struct CreateDocumentResponse {
    code: i32,
    msg: String,
    data: Option<CreateDocumentData>,
}

#[derive(Deserialize)]
struct CreateDocumentData {
    document: Option<DocumentInfo>,
}

#[derive(Deserialize)]
struct DocumentInfo {
    document_id: String,
}

#[derive(Serialize)]
struct CreateBlockRequest {
    children: Vec<Block>,
}

#[derive(Serialize)]
struct Block {
    block_type: i32,
    text: TextBlock,
}

#[derive(Serialize)]
struct TextBlock {
    elements: Vec<TextElement>,
}

#[derive(Serialize)]
struct TextElement {
    text_run: TextRun,
}

#[derive(Serialize)]
struct TextRun {
    content: String,
}

#[derive(Deserialize)]
struct ApiResponse {
    code: i32,
    msg: String,
}

impl FeishuClient {
    pub fn new(app_id: String, app_secret: String) -> Self {
        Self {
            http: Client::new(),
            app_id,
            app_secret,
            token_cache: Mutex::new(None),
        }
    }

    /// Update credentials (when settings change).
    pub fn update_credentials(&self, app_id: &str, app_secret: &str) -> bool {
        if app_id == self.app_id && app_secret == self.app_secret {
            return false;
        }
        // Invalidate cache — caller should create a new client
        true
    }

    /// Get a valid tenant_access_token, refreshing if needed.
    pub async fn get_token(&self) -> Result<String, String> {
        // Check cache
        {
            let cache = self.token_cache.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(ref cached) = *cache {
                if Instant::now() < cached.expires_at {
                    return Ok(cached.token.clone());
                }
            }
        }

        // Refresh
        let resp = self
            .http
            .post(format!("{BASE_URL}/auth/v3/tenant_access_token/internal"))
            .json(&TokenRequest {
                app_id: self.app_id.clone(),
                app_secret: self.app_secret.clone(),
            })
            .send()
            .await
            .map_err(|e| format!("feishu auth request failed: {e}"))?;

        let body: TokenResponse = resp
            .json()
            .await
            .map_err(|e| format!("feishu auth parse failed: {e}"))?;

        if body.code != 0 {
            return Err(format!("feishu auth error {}: {}", body.code, body.msg));
        }

        let token = body
            .tenant_access_token
            .ok_or("feishu auth: no token in response")?;
        let expire_secs = body.expire.unwrap_or(7200);
        let expires_at =
            Instant::now() + std::time::Duration::from_secs(expire_secs - TOKEN_REFRESH_MARGIN_SECS);

        // Cache
        {
            let mut cache = self.token_cache.lock().unwrap_or_else(|e| e.into_inner());
            *cache = Some(CachedToken {
                token: token.clone(),
                expires_at,
            });
        }

        debug!("[feishu] token refreshed, expires in {expire_secs}s");
        Ok(token)
    }

    /// Create a new document in the specified folder.
    pub async fn create_document(
        &self,
        title: &str,
        folder_token: Option<&str>,
    ) -> Result<String, String> {
        let token = self.get_token().await?;

        let resp = self
            .http
            .post(format!("{BASE_URL}/docx/v1/documents"))
            .bearer_auth(&token)
            .json(&CreateDocumentRequest {
                title: title.to_string(),
                folder_token: folder_token.map(|s| s.to_string()),
            })
            .send()
            .await
            .map_err(|e| format!("feishu create doc failed: {e}"))?;

        let body: CreateDocumentResponse = resp
            .json()
            .await
            .map_err(|e| format!("feishu create doc parse failed: {e}"))?;

        if body.code != 0 {
            return Err(format!(
                "feishu create doc error {}: {}",
                body.code, body.msg
            ));
        }

        let doc_id = body
            .data
            .and_then(|d| d.document)
            .map(|d| d.document_id)
            .ok_or("feishu create doc: no document_id in response")?;

        debug!("[feishu] created document: {doc_id}");
        Ok(doc_id)
    }

    /// Append text to a document (adds a text block at the end).
    pub async fn append_text(&self, document_id: &str, text: &str) -> Result<(), String> {
        if text.trim().is_empty() {
            return Ok(());
        }

        let token = self.get_token().await?;

        // Append to document root (block_id = document_id)
        let url = format!(
            "{BASE_URL}/docx/v1/documents/{document_id}/blocks/{document_id}/children"
        );

        let block = Block {
            block_type: 2, // text block
            text: TextBlock {
                elements: vec![TextElement {
                    text_run: TextRun {
                        content: text.to_string(),
                    },
                }],
            },
        };

        let resp = self
            .http
            .post(&url)
            .bearer_auth(&token)
            .json(&CreateBlockRequest {
                children: vec![block],
            })
            .send()
            .await
            .map_err(|e| format!("feishu append failed: {e}"))?;

        let body: ApiResponse = resp
            .json()
            .await
            .map_err(|e| format!("feishu append parse failed: {e}"))?;

        if body.code != 0 {
            return Err(format!("feishu append error {}: {}", body.code, body.msg));
        }

        debug!("[feishu] appended {} chars to doc {document_id}", text.len());
        Ok(())
    }
}
