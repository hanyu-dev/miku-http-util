//! `axum` integration for [`OwnedQuery`](OwnedQuery).

use axum::{extract::Request, handler::Handler};

use super::utils::{ParseQueryError, ParseQueryResult};
use crate::request::parser::OwnedQuery;

#[macro_export]
/// Just [`WithQueryHandler::new`].
macro_rules! query_keys_required {
    ($handler:expr => $required:expr) => {
        $crate::request::parser::integration::WithQueryHandler::new($handler, $required)
    };
}

#[derive(Debug, Clone, Copy)]
/// Wrapper over handler
pub struct WithQueryHandler<H> {
    inner: H,
    required: &'static [&'static str],
}

impl<H> WithQueryHandler<H> {
    /// Create a new [`WithQueryHandler`].
    pub const fn new(inner: H, required: &'static [&'static str]) -> Self {
        Self { inner, required }
    }
}

impl<H, T, S> Handler<T, S> for WithQueryHandler<H>
where
    H: Handler<T, S>,
{
    type Future = H::Future;

    fn call(self, mut req: Request, state: S) -> Self::Future {
        if let Some(owned_query) = req.uri().query().map(OwnedQuery::parse) {
            #[cfg(feature = "feat-tracing")]
            tracing::trace!("Found query: {:?}", owned_query);

            let owned_query = self
                .required
                .iter()
                .find_map(|&key| {
                    if !owned_query.contains_key(key) {
                        #[cfg(feature = "feat-tracing")]
                        tracing::error!(key, "Missing query key");

                        Some(ParseQueryResult::Err(ParseQueryError::MissingKey(key)))
                    } else {
                        None
                    }
                })
                .unwrap_or(ParseQueryResult::Ok(owned_query));

            req.extensions_mut().insert(owned_query);
        }

        self.inner.call(req, state)
    }
}

#[cfg(test)]
mod test {
    use axum::{extract::Request, response::IntoResponse, routing::get, Router};

    #[test]
    fn test() {
        let _app: Router<()> = Router::new()
            .route("/", get(test_router))
            .route("/test", get(query_keys_required!(test_router => &["hey"])));
    }

    async fn test_router(_request: Request) -> impl IntoResponse {
        "Hello world!"
    }
}
