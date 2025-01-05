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
    #[cfg(feature = "feat-response-ext-json")]
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
                tracing::error!("Failed to parse JSON: {e:?}");
                Err(self)
            }
        }
    }

    #[cfg(feature = "feat-integrate-rquest")]
    /// Helper to convert a [`rquest::Response`] to a [`ResponseExt`]
    pub async fn from_rquest_response(response: rquest::Response) -> anyhow::Result<Self> {
        use http_body_util::BodyExt;

        let response: http::Response<rquest::Body> = response.into();

        let (response_parts, body) = response.into_parts();

        Ok(ResponseExt {
            response_parts,
            body: BodyExt::collect(body).await.map(|buf| buf.to_bytes())?,
        })
    }
}
