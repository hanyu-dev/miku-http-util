//! HTTP request utilities: builder related.

use std::{borrow::Cow, convert::Infallible, ops};

use macro_toolset::{
    md5, str_concat_v2 as str_concat,
    string_v2::{general::tuple::SeplessTuple, PushAnyT, StringExtT},
    urlencoding_str,
};

#[derive(Debug)]
#[repr(transparent)]
/// Helper for query string building.
pub struct Queries<'q> {
    inner: Vec<(Cow<'q, str>, Cow<'q, str>)>,
}

impl<'q> ops::Deref for Queries<'q> {
    type Target = Vec<(Cow<'q, str>, Cow<'q, str>)>;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<'q> Queries<'q> {
    #[inline]
    /// Create a new empty query string builder.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: Vec::with_capacity(capacity),
        }
    }

    #[inline]
    /// Push a new key-value pair into the query string builder.
    pub fn push(mut self, key: impl Into<Cow<'q, str>>, value: impl Into<Cow<'q, str>>) -> Self {
        self.inner.push((key.into(), value.into()));
        self
    }

    #[inline]
    /// Push a new key-value pair into the query string builder.
    pub fn push_any(mut self, key: impl Into<Cow<'q, str>>, value: impl StringExtT) -> Self {
        self.inner.push((key.into(), value.to_string_ext().into()));
        self
    }

    #[inline]
    /// Sort the query pairs by key.
    pub fn sorted(mut self) -> Self {
        self.inner.sort_unstable_by(|l, r| l.0.cmp(&r.0));
        self
    }

    #[inline]
    /// Get inner query pairs.
    pub const fn inner(&self) -> &Vec<(Cow<'q, str>, Cow<'q, str>)> {
        &self.inner
    }

    #[inline]
    /// Get inner query pairs.
    pub fn into_inner(self) -> Vec<(Cow<'q, str>, Cow<'q, str>)> {
        self.inner
    }

    #[inline]
    /// Build the query string, unsigned.
    pub fn build(self) -> String {
        str_concat!(sep = "&"; self.inner.iter().map(|(k, v)| {
            (k, "=", urlencoding_str!(E: v))
        }))
    }

    #[inline]
    /// Build the query string with given signer.
    pub fn build_signed<S: SignerT>(self, signer: S) -> Result<String, S::Error> {
        signer.build_signed(self)
    }
}

/// Helper trait for query string signing.
pub trait SignerT {
    /// The error type.
    type Error;

    /// Sign the query string and return the final query string.
    fn build_signed(self, queries: Queries) -> Result<String, Self::Error>;
}

#[derive(Debug, Clone, Copy)]
/// Helper for query string signing: MD5.
pub struct Md5Signer<'s> {
    /// The query param key.
    ///
    /// The default is `"sign"`.
    pub query_key: &'s str,

    /// The salt to be used for signing (prefix).
    pub prefix_salt: Option<&'s str>,

    /// The salt to be used for signing (suffix).
    pub suffix_salt: Option<&'s str>,
}

impl Default for Md5Signer<'_> {
    fn default() -> Self {
        Self {
            query_key: "sign",
            prefix_salt: None,
            suffix_salt: None,
        }
    }
}

impl SignerT for Md5Signer<'_> {
    type Error = Infallible;
    fn build_signed(self, queries: Queries) -> Result<String, Self::Error> {
        let queries = queries.sorted();

        let mut final_string_buf = String::with_capacity(64);

        final_string_buf.push_any_with_separator(
            queries
                .inner
                .iter()
                .map(|(k, v)| SeplessTuple::new((k, "=", urlencoding_str!(E: v)))),
            "&",
        );

        let signed = match (self.prefix_salt, self.suffix_salt) {
            (None, Some(suffix_salt)) => md5!(final_string_buf, suffix_salt), // most frequent
            (None, None) => md5!(final_string_buf),
            (Some(prefix_salt), Some(suffix_salt)) => {
                md5!(prefix_salt, final_string_buf, suffix_salt)
            }
            (Some(prefix_salt), None) => md5!(prefix_salt, final_string_buf),
        };

        if final_string_buf.is_empty() {
            final_string_buf.push_any((self.query_key, "=", signed.as_str()));
        } else {
            final_string_buf.push_any(("&", self.query_key, "=", signed.as_str()));
        }

        Ok(final_string_buf)
    }
}

impl<'s> Md5Signer<'s> {
    #[inline]
    /// Create a new MD5 signer.
    pub const fn new(
        query_key: &'s str,
        prefix_salt: Option<&'s str>,
        suffix_salt: Option<&'s str>,
    ) -> Self {
        Self {
            query_key,
            prefix_salt,
            suffix_salt,
        }
    }

    #[inline]
    /// Create a new MD5 signer with the default query key.
    pub const fn new_default() -> Self {
        Self {
            query_key: "sign",
            prefix_salt: None,
            suffix_salt: None,
        }
    }

    #[inline]
    /// Set the query key.
    pub const fn with_query_key(self, query_key: &'s str) -> Self {
        Self { query_key, ..self }
    }

    #[inline]
    /// Add a prefix salt to the signer.
    pub const fn with_prefix_salt(self, prefix_salt: Option<&'s str>) -> Self {
        Self {
            prefix_salt,
            ..self
        }
    }

    #[inline]
    /// Add a suffix salt to the signer.
    pub const fn with_suffix_salt(self, suffix_salt: Option<&'s str>) -> Self {
        Self {
            suffix_salt,
            ..self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_general() {
        let queries = Queries::with_capacity(16)
            .push_any("test1", 1)
            .push_any("test2", "2")
            .build_signed(Md5Signer::new_default().with_suffix_salt(Some("0123456789abcdef")))
            .unwrap();

        assert_eq!(
            queries,
            "test1=1&test2=2&sign=cc4f5844a6a1893a88d648cebba5462f"
        )
    }
}
