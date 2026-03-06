//! Supabase Storage operations for file uploads/downloads.

use crate::{Error, Result, SupabaseClient};

impl SupabaseClient {
    /// Upload a file to a storage bucket.
    ///
    /// # CLI Usage
    ///
    /// ```bash
    /// logline storage upload --bucket artifacts --path ws/tenant-123/report.pdf ./report.pdf
    /// ```
    ///
    /// # Example
    ///
    /// ```ignore
    /// let url = client
    ///     .upload("artifacts", "ws/tenant-123/report.pdf", &file_bytes)
    ///     .await?;
    /// println!("Uploaded to: {}", url);
    /// ```
    pub async fn upload(&self, bucket: &str, path: &str, data: &[u8]) -> Result<String> {
        let url = format!("{}/object/{}/{}", self.storage_url(), bucket, path);

        let response = self
            .http()
            .post(&url)
            .headers(self.auth_headers())
            .header("Content-Type", "application/octet-stream")
            .body(data.to_vec())
            .send()
            .await?;

        if !response.status().is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(Error::Storage(format!("Upload failed: {}", text)));
        }

        // Return public URL
        Ok(format!(
            "{}/object/public/{}/{}",
            self.storage_url(),
            bucket,
            path
        ))
    }

    /// Download a file from a storage bucket.
    ///
    /// # CLI Usage
    ///
    /// ```bash
    /// logline storage download --bucket artifacts --path ws/tenant-123/report.pdf ./report.pdf
    /// ```
    pub async fn download(&self, bucket: &str, path: &str) -> Result<Vec<u8>> {
        let url = format!("{}/object/{}/{}", self.storage_url(), bucket, path);

        let response = self
            .http()
            .get(&url)
            .headers(self.auth_headers())
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(Error::Storage(format!(
                "Download failed ({}): {}",
                status, text
            )));
        }

        Ok(response.bytes().await?.to_vec())
    }

    /// Delete a file from a storage bucket.
    ///
    /// # CLI Usage
    ///
    /// ```bash
    /// logline storage delete --bucket artifacts --path ws/tenant-123/report.pdf
    /// ```
    pub async fn delete_file(&self, bucket: &str, path: &str) -> Result<()> {
        let url = format!("{}/object/{}/{}", self.storage_url(), bucket, path);

        let response = self
            .http()
            .delete(&url)
            .headers(self.auth_headers())
            .send()
            .await?;

        if !response.status().is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(Error::Storage(format!("Delete failed: {}", text)));
        }

        Ok(())
    }

    /// List files in a bucket path.
    ///
    /// # CLI Usage
    ///
    /// ```bash
    /// logline storage list --bucket artifacts --prefix ws/tenant-123/
    /// ```
    pub async fn list_files(&self, bucket: &str, prefix: &str) -> Result<Vec<StorageObject>> {
        let url = format!("{}/object/list/{}", self.storage_url(), bucket);

        let response = self
            .http()
            .post(&url)
            .headers(self.auth_headers())
            .json(&serde_json::json!({
                "prefix": prefix,
                "limit": 1000
            }))
            .send()
            .await?;

        if !response.status().is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(Error::Storage(format!("List failed: {}", text)));
        }

        Ok(response.json().await?)
    }

    /// Get a signed URL for temporary public access.
    ///
    /// # CLI Usage
    ///
    /// ```bash
    /// logline storage sign --bucket artifacts --path ws/tenant-123/report.pdf --expires 3600
    /// ```
    pub async fn sign_url(&self, bucket: &str, path: &str, expires_in: u64) -> Result<String> {
        let url = format!("{}/object/sign/{}/{}", self.storage_url(), bucket, path);

        let response = self
            .http()
            .post(&url)
            .headers(self.auth_headers())
            .json(&serde_json::json!({
                "expiresIn": expires_in
            }))
            .send()
            .await?;

        if !response.status().is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(Error::Storage(format!("Sign failed: {}", text)));
        }

        let result: serde_json::Value = response.json().await?;
        result["signedURL"]
            .as_str()
            .map(|s| format!("{}{}", self.url(), s))
            .ok_or_else(|| Error::Storage("No signedURL in response".into()))
    }
}

/// A storage object (file or folder).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StorageObject {
    pub name: String,
    pub id: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub last_accessed_at: Option<String>,
    pub metadata: Option<serde_json::Value>,
}
