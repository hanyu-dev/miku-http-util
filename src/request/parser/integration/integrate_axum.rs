//! `axum` integration for [`OwnedQuery`](OwnedQuery).

use axum::{extract::Request, handler::Handler};

use super::parse_query;

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
        parse_query(&mut req, self.required);

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
