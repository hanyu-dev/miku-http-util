//! HTTP response utilities

use bytes::Bytes;
use http::response::Parts;

#[derive(Debug, Clone)]
/// Response (Extended)
pub struct ResponseExt<B = Bytes> {
    /// HTTP response parts (see [`http::response::Parts`])
    pub response_parts: Parts,

    /// Body bytes
    pub body: B,
}

impl ResponseExt {
    /// Convert the body to a JSON value
    ///
    /// If the body is not valid JSON, the original response is returned as an
    /// error.
    pub fn json<T>(self) -> Result<ResponseExt<T>, Self>
    where
        T: for<'a> serde::Deserialize<'a>,
    {
        match serde_json::from_slice(&self.body) {
            Ok(body) => Ok(ResponseExt {
                response_parts: self.response_parts,
                body,
            }),
            Err(e) => {
                #[cfg(feature = "feat-tracing")]
                tracing::error!("Failed to parse JSON: {}", e);
                Err(self)
            }
        }
    }
}
