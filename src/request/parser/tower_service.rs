//! `tower` integration for [`OwnedQueries`].

use std::{
    marker::PhantomData,
    task::{Context, Poll},
};

use anyhow::Result;
use http::Request;
use tower_layer::Layer;
use tower_service::Service;

use super::OwnedQueries;

#[inline]
/// Helper function to extract [`OwnedQueries`] from
/// [`Extensions`](http::Extensions) within given [`Request`].
pub fn get_queries<ReqBody>(request: &Request<ReqBody>) -> Result<Option<&OwnedQueries>> {
    match request
        .extensions()
        .get::<Result<OwnedQueries, ParseQueriesError>>()
    {
        Some(Ok(data)) => Ok(Some(data)),
        Some(Err(e)) => Err((*e).into()),
        None => Ok(None),
    }
}

#[derive(Debug, Clone, Copy)]
#[derive(thiserror::Error)]
enum ParseQueriesError {
    #[error("missing query key `{0}`")]
    MissingKey(&'static str),
}

#[derive(Debug, Default, Copy)]
#[repr(transparent)]
/// [`Layer`] for parsing [`OwnedQueries`] from a [`Request`] and insert into
/// the [`Request`] extensions.
pub struct QueriesLayer<ReqBody> {
    _req_body: PhantomData<ReqBody>,
    required: &'static [&'static str],
}

// `ReqBody`, `ResBody` is just type markers, we actually don't care
// about what actually it is, but the compiler will complain that *`Clone` is
// needed* if we just `#[derive(Clone)]`
impl<ReqBody> Clone for QueriesLayer<ReqBody> {
    fn clone(&self) -> Self {
        Self {
            _req_body: PhantomData,
            required: self.required,
        }
    }
}

#[allow(unsafe_code)]
// SAFETY: `ReqBody`, `ResBody` is just type markers, we actually don't care
// about what actually it is, but compiler complains about `the type parameter
// `B` is not constrained by ***`.
unsafe impl<ReqBody> Sync for QueriesLayer<ReqBody> {}

impl<ReqBody> QueriesLayer<ReqBody> {
    /// Create a new [`QueriesLayer`].
    ///
    /// # Params
    ///
    /// - `required`: required query keys
    pub const fn new(required: &'static [&'static str]) -> Self {
        Self {
            _req_body: PhantomData,
            required,
        }
    }
}

impl<S, ReqBody> Layer<S> for QueriesLayer<ReqBody>
where
    S: Service<Request<ReqBody>> + Send + 'static,
{
    type Service = QueriesServcie<S, ReqBody>;

    fn layer(&self, inner: S) -> Self::Service {
        QueriesServcie {
            inner,
            required: self.required,
            _req_body: PhantomData,
        }
    }
}

#[derive(Debug)]
/// [`Service`] for parsing [`OwnedQueries`] from a [`Request`] and insert into
/// the [`Request`] extensions.
pub struct QueriesServcie<S, ReqBody> {
    inner: S,
    required: &'static [&'static str],
    _req_body: PhantomData<ReqBody>,
}

// `ReqBody`, `ResBody` is just type markers, we actually don't care
// about what actually it is, but the compiler will complain that *`Clone` is
// needed* if we just `#[derive(Clone)]`
impl<S, ReqBody> Clone for QueriesServcie<S, ReqBody>
where
    S: Clone,
{
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            required: self.required,
            _req_body: PhantomData,
        }
    }
}

#[allow(unsafe_code)]
// SAFETY: `ReqBody`, `ResBody` is just type markers, we actually don't care
// about what actually it is, but compiler complains about `the type parameter
// `B` is not constrained by ***`.
unsafe impl<S, ReqBody> Sync for QueriesServcie<S, ReqBody> where S: Sync {}

impl<S, ReqBody> Service<Request<ReqBody>> for QueriesServcie<S, ReqBody>
where
    S: Service<Request<ReqBody>> + Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: Request<ReqBody>) -> Self::Future {
        if let Some(owned_queries) = req.uri().query().map(OwnedQueries::parse) {
            #[cfg(feature = "feat-tracing")]
            tracing::trace!("Found queries: {:?}", owned_queries);

            let mut has_error = false;
            for &key in self.required {
                if !owned_queries.contains_key(key) {
                    #[cfg(feature = "feat-tracing")]
                    tracing::error!(key, "Missing query key");

                    has_error = true;
                    req.extensions_mut()
                        .insert(Err::<OwnedQueries, _>(ParseQueriesError::MissingKey(key)));

                    break;
                }
            }

            if !has_error {
                req.extensions_mut()
                    .insert(Ok::<_, ParseQueriesError>(owned_queries));
            }
        }

        self.inner.call(req)
    }
}
