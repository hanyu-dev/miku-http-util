//! Integration with other crates, utils

use anyhow::Result;
use http::Request;

use crate::request::parser::OwnedQuery;

/// Type alias for [`Result<OwnedQuery, ParseQueryError>`].
///
/// You may just need [`get_query`] to extract parsed [`Query`](OwnedQuery) from
/// [`Extensions`](http::Extensions) within given [`Request`].
pub type ParseQueryResult = Result<OwnedQuery, ParseQueryError>;

#[inline]
/// Helper function to extract parsed [`Query`](OwnedQuery) from
/// [`Extensions`](http::Extensions) within given [`Request`].
pub fn get_query<ReqBody>(request: &Request<ReqBody>) -> Result<Option<&OwnedQuery>> {
    match request.extensions().get::<ParseQueryResult>() {
        Some(Ok(data)) => Ok(Some(data)),
        Some(Err(e)) => Err((*e).into()),
        None => Ok(None),
    }
}

#[derive(Debug, Clone, Copy)]
#[derive(thiserror::Error)]
/// `ParseQueryError`
pub enum ParseQueryError {
    #[error("missing query key `{0}`")]
    /// Missing required query key
    MissingKey(&'static str),
}
