//! HTTP request utilities: parser related.

use std::{borrow::Cow, collections::HashMap, sync::Arc};

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
                .unwrap_or_else(|| {
                    #[cfg(feature = "feat-tracing")]
                    tracing::warn!("Failed to parse `{query}`");

                    EStr::EMPTY
                })
                .split('&')
                .map(|pair| {
                    pair.split_once('=').unwrap_or_else(|| {
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

    #[inline]
    /// Parse query string.
    pub fn parse(query: &str) -> Self {
        use fluent_uri::encoding::{encoder::IQuery, EStr};

        Self {
            inner: EStr::<IQuery>::new(query)
                .unwrap_or_else(|| {
                    #[cfg(feature = "feat-tracing")]
                    tracing::warn!("Failed to parse `{query}`");

                    EStr::EMPTY
                })
                .split('&')
                .map(|pair| {
                    pair.split_once('=').unwrap_or_else(|| {
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
