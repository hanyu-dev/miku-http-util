//! HTTP request utilities: parser related.

#[cfg(feature = "feat-integrate-tower")]
pub mod tower_service;

use std::{
    borrow::{Borrow, Cow},
    collections::HashMap,
    hash::Hash,
    sync::Arc,
};

use macro_toolset::wrapper;

wrapper! {
    #[derive(Debug, Clone)]
    /// Helper for query string parsing.
    ///
    /// You may also need [`OwnedQueries`].
    pub Queries<'q>(HashMap<Cow<'q, str>, Cow<'q, str>, foldhash::fast::RandomState>)
}

impl<'q> Queries<'q> {
    #[cfg(feature = "feat-integrate-http")]
    #[inline]
    /// Parse query string from [`http::Uri`].
    pub fn parse_uri(uri: &'q http::Uri) -> Option<Self> {
        uri.query().map(Self::parse)
    }

    #[inline]
    /// Parse query string.
    pub fn parse(query: &'q str) -> Self {
        use fluent_uri::encoding::{encoder::IQuery, EStr};

        Self {
            inner: EStr::<IQuery>::new(query)
                .unwrap_or({
                    #[cfg(feature = "feat-tracing")]
                    tracing::warn!("Failed to parse `{query}`");

                    EStr::EMPTY
                })
                .split('&')
                .map(|pair| {
                    pair.split_once('=').unwrap_or({
                        #[cfg(feature = "feat-tracing")]
                        tracing::warn!("Failed to split query pair: {:?}", pair);

                        (pair, EStr::EMPTY)
                    })
                })
                .map(|(k, v)| {
                    (
                        k.decode().into_string_lossy(),
                        v.decode().into_string_lossy(),
                    )
                })
                .collect::<HashMap<_, _, _>>(),
        }
    }
}

wrapper! {
    #[derive(Debug, Clone)]
    /// Helper for query string parsing.
    ///
    /// You may also need [`Queries`] if you just want a borrowed version.
    pub OwnedQueries(Arc<HashMap<Arc<str>, Arc<str>, foldhash::fast::RandomState>>)
}

impl OwnedQueries {
    #[cfg(feature = "feat-integrate-http")]
    #[inline]
    /// Parse query string from [`http::Uri`].
    pub fn parse_uri(uri: &http::Uri) -> Option<Self> {
        uri.query().map(Self::parse)
    }

    #[allow(clippy::multiple_bound_locations)]
    #[inline]
    /// Since [`OwnedQueries`] is a wrapper of [`Arc<HashMap<Arc<str>,
    /// Arc<str>>>`] and implements `Deref`, without this you can still call
    /// [`HashMap::get`] (though auto deref), however you will get an
    /// `Option<&Arc<str>>`, and `&Arc<str>` is probably not what you want.
    ///
    /// Here's an example:
    ///
    /// ```no_run
    /// let data: OwnedQueries = ...;
    /// let example = data.get("example").unwrap(); // &Arc<str>
    /// assert!(example, "example");
    /// ```
    ///
    /// `assert!(example, "example")` will not compile at all, you must change
    /// it to `assert!(&**example, "example")`:
    ///
    /// ```no_run
    /// & * *example
    /// ││└ &Arc<str> deref to Arc<str>
    /// │└ Arc<str> deref to str
    /// └ &str
    /// ```
    ///
    /// This is really not convenient and graceful, so we provide this method as
    /// an replacement of [`HashMap::get`].
    /// See [*The Rustonomicon - The Dot Operator*](https://doc.rust-lang.org/nomicon/dot-operator.html) for the reason why we can do so.
    pub fn get<Q: ?Sized>(&self, k: &Q) -> Option<&str>
    where
        Arc<str>: Borrow<Q>,
        Q: Hash + Eq,
    {
        self.inner.get(k).map(|v| &**v)
    }

    #[inline]
    /// Parse query string.
    pub fn parse(query: &str) -> Self {
        use fluent_uri::encoding::{encoder::IQuery, EStr};

        Self {
            inner: EStr::<IQuery>::new(query)
                .unwrap_or({
                    #[cfg(feature = "feat-tracing")]
                    tracing::warn!("Failed to parse `{query}`");

                    EStr::EMPTY
                })
                .split('&')
                .map(|pair| {
                    pair.split_once('=').unwrap_or({
                        #[cfg(feature = "feat-tracing")]
                        tracing::warn!("Failed to split query pair: {:?}", pair);

                        (pair, EStr::EMPTY)
                    })
                })
                .map(|(k, v)| {
                    (
                        k.decode().into_string_lossy().into(),
                        v.decode().into_string_lossy().into(),
                    )
                })
                .collect::<HashMap<_, _, _>>()
                .into(),
        }
    }
}
