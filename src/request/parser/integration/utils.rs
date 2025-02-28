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

#[inline]
pub(super) fn parse_query<ReqBody>(req: &mut Request<ReqBody>, required: &'static [&'static str]) {
    match req.uri().query().map(OwnedQuery::parse) {
        Some(owned_query) => {
            #[cfg(feature = "feat-tracing")]
            tracing::trace!("Found query: {:?}", owned_query);

            let owned_query = required
                .iter()
                .find_map(|&key| {
                    if !owned_query.contains_key(key) {
                        #[cfg(feature = "feat-tracing")]
                        tracing::error!(key, "Missing query key.");

                        Some(ParseQueryResult::Err(ParseQueryError::MissingKey(key)))
                    } else {
                        None
                    }
                })
                .unwrap_or(ParseQueryResult::Ok(owned_query));

            req.extensions_mut().insert::<ParseQueryResult>(owned_query);
        }
        None => {
            if !required.is_empty() {
                #[cfg(feature = "feat-tracing")]
                tracing::error!("Missing query.");

                req.extensions_mut()
                    .insert::<ParseQueryResult>(ParseQueryResult::Err(
                        ParseQueryError::MissingKey(required[0]),
                    ));
            }
        }
    }
}
