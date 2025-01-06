//! `tower` integration for [`OwnedQueries`].

use std::{
    marker::PhantomData,
    task::{Context, Poll},
};

use http::{Request, Response};
use tower_layer::Layer;
use tower_service::Service;

use super::OwnedQueries;

#[derive(Debug, Default)]
/// [`Layer`] for parsing [`OwnedQueries`] from a [`Request`] and insert into
/// the [`Request`] extensions.
pub struct QueriesLayer<ReqBody, ResBody> {
    _req_body: PhantomData<ReqBody>,
    _res_body: PhantomData<ResBody>,
}

impl<ReqBody, ResBody> Clone for QueriesLayer<ReqBody, ResBody> {
    fn clone(&self) -> Self {
        Self {
            _req_body: PhantomData,
            _res_body: PhantomData,
        }
    }
}

#[allow(unsafe_code, reason = "")]
unsafe impl<ReqBody, ResBody> Sync for QueriesLayer<ReqBody, ResBody> {}

impl<ReqBody, ResBody> QueriesLayer<ReqBody, ResBody> {
    /// Create a new [`QueriesLayer`].
    pub const fn new() -> Self {
        Self {
            _req_body: PhantomData,
            _res_body: PhantomData,
        }
    }
}

impl<S, ReqBody, ResBody> Layer<S> for QueriesLayer<ReqBody, ResBody>
where
    S: Service<Request<ReqBody>, Response = Response<ResBody>> + Send + 'static,
{
    type Service = QueriesServcie<S, ReqBody, ResBody>;

    fn layer(&self, inner: S) -> Self::Service {
        QueriesServcie {
            inner,
            _req_body: PhantomData,
            _res_body: PhantomData,
        }
    }
}

#[derive(Debug)]
/// [`Service`] for parsing [`OwnedQueries`] from a [`Request`] and insert into
/// the [`Request`] extensions.
pub struct QueriesServcie<S, ReqBody, ResBody> {
    inner: S,
    _req_body: PhantomData<ReqBody>,
    _res_body: PhantomData<ResBody>,
}

impl<S, ReqBody, ResBody> Clone for QueriesServcie<S, ReqBody, ResBody>
where
    S: Clone,
{
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            _req_body: PhantomData,
            _res_body: PhantomData,
        }
    }
}

#[allow(unsafe_code, reason = "")]
unsafe impl<S, ReqBody, ResBody> Sync for QueriesServcie<S, ReqBody, ResBody> where S: Sync {}

impl<S, ReqBody, ResBody> Service<Request<ReqBody>> for QueriesServcie<S, ReqBody, ResBody>
where
    S: Service<Request<ReqBody>, Response = Response<ResBody>> + Send + 'static,
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

            req.extensions_mut().insert(owned_queries);
        }

        self.inner.call(req)
    }
}
