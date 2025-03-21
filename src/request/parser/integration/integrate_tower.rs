//! `tower` integration for [`OwnedQuery`](OwnedQuery).

use std::{
    marker::PhantomData,
    task::{Context, Poll},
};

use anyhow::Result;
use http::Request;
use tower_layer::Layer;
use tower_service::Service;

use super::parse_query;

#[deprecated(since = "0.6.0", note = "Renamed, use `WithQueryLayer` instead.")]
/// Renamed, use [`WithQueryLayer`] instead.
pub type QueriesLayer<ReqBody> = WithQueryLayer<ReqBody>;

#[deprecated(since = "0.6.0", note = "Renamed, use `WithQueryService` instead.")]
/// Renamed, use [`WithQueryLayer`] instead.
pub type QueriesServcie<S, ReqBody> = WithQueryService<S, ReqBody>;

#[derive(Debug, Default, Copy)]
#[repr(transparent)]
/// [`Layer`] for parsing [`OwnedQuery`] from a [`Request`] and insert into
/// the [`Request`] extensions.
pub struct WithQueryLayer<ReqBody> {
    _req_body: PhantomData<ReqBody>,
    required: &'static [&'static str],
}

// `ReqBody`, `ResBody` is just type markers, we actually don't care
// about what actually it is, but the compiler will complain that *`Clone` is
// needed* if we just `#[derive(Clone)]`
impl<ReqBody> Clone for WithQueryLayer<ReqBody> {
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
unsafe impl<ReqBody> Sync for WithQueryLayer<ReqBody> {}

impl<ReqBody> WithQueryLayer<ReqBody> {
    /// Create a new [`WithQueryLayer`].
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

impl<S, ReqBody> Layer<S> for WithQueryLayer<ReqBody>
where
    S: Service<Request<ReqBody>> + Send + 'static,
{
    type Service = WithQueryService<S, ReqBody>;

    fn layer(&self, inner: S) -> Self::Service {
        WithQueryService {
            inner,
            required: self.required,
            _req_body: PhantomData,
        }
    }
}

#[derive(Debug)]
/// [`Service`] for parsing [`OwnedQuery`] from a [`Request`] and insert into
/// the [`Request`] extensions.
pub struct WithQueryService<S, ReqBody> {
    inner: S,
    required: &'static [&'static str],
    _req_body: PhantomData<ReqBody>,
}

impl<S, ReqBody> WithQueryService<S, ReqBody> {
    /// Create a new [`WithQueryService`].
    ///
    /// # Params
    ///
    /// - `required`: required query keys
    pub const fn new(inner: S, required: &'static [&'static str]) -> Self {
        Self {
            inner,
            required,
            _req_body: PhantomData,
        }
    }
}

// `ReqBody`, `ResBody` is just type markers, we actually don't care
// about what actually it is, but the compiler will complain that *`Clone` is
// needed* if we just `#[derive(Clone)]`
impl<S, ReqBody> Clone for WithQueryService<S, ReqBody>
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
unsafe impl<S, ReqBody> Sync for WithQueryService<S, ReqBody> where S: Sync {}

impl<S, ReqBody> Service<Request<ReqBody>> for WithQueryService<S, ReqBody>
where
    S: Service<Request<ReqBody>> + Send + 'static,
{
    type Error = S::Error;
    type Future = S::Future;
    type Response = S::Response;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: Request<ReqBody>) -> Self::Future {
        parse_query(&mut req, self.required);

        self.inner.call(req)
    }
}
