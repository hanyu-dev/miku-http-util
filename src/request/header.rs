//! HTTP request utilities: HTTP header related.

use anyhow::{anyhow, Result};
use http::{
    header::{AsHeaderName, InvalidHeaderValue},
    HeaderMap, HeaderName, HeaderValue,
};
use macro_toolset::{
    b64_decode, b64_encode_bytes, base64::engine::general_purpose::STANDARD_NO_PAD,
    string_v2::StringExtT,
};

/// Trait helper for managing HTTP header keys.
pub trait HeaderKeyExtT {
    /// `as_str` and most times should be &'static
    fn as_str(&self) -> &str;

    /// Get the key name
    fn as_header_name(&self) -> HeaderName;

    /// Get default value of the key
    ///
    /// Return `None` if no default one.
    fn default_header_value(&self) -> Option<HeaderValue> {
        None
    }

    /// Get if is an binary type gRPC Metadata key.
    ///
    /// By default we treat keys ending with `-bin` as binary keys.
    fn is_binary_key(&self) -> bool {
        self.as_str().ends_with("-bin")
    }
}

impl HeaderKeyExtT for &'static str {
    #[inline]
    fn as_str(&self) -> &str {
        self
    }

    #[inline]
    fn as_header_name(&self) -> HeaderName {
        HeaderName::from_static(self)
    }
}

/// Trait for extending [`http::HeaderMap`]'s methods.
///
/// If `T` implements this trait, `&mut T` will also implement this trait.
pub trait HeaderMapExtT {
    #[inline]
    /// Returns a reference to the value associated with the key.
    ///
    /// For gRPC metadata, please use [`MetadataMapExtT::get_bin`] instead.
    ///
    /// Key **SHOULD NOT** be a binary type gRPC Metadata key, though for
    /// performance consideration, we will not check so.
    ///
    /// Notice: if value contains invalid header value characters(non-ascii), it
    /// will be ignored and return `None`.
    fn get_ascii(&self, key: impl HeaderKeyExtT) -> Option<&str> {
        self.get_exact(key.as_header_name()).and_then(|v| {
            v.to_str()
                .inspect_err(|e| {
                    #[cfg(feature = "feat-tracing")]
                    tracing::warn!("Invalid header value [{v:?}]: {e:?}");
                })
                .ok()
        })
    }

    /// Insert general http header
    ///
    /// For gRPC metadata, please instead use [`MetadataMapExtT::insert_bin`] or
    /// [`MetadataMapExtT::insert_bin_byte`].
    ///
    /// Key **SHOULD NOT** be a binary type gRPC Metadata key, though for
    /// performance consideration, we will not check so.
    ///
    /// # Error
    ///
    /// - [`InvalidHeaderValue`] if the value contains invalid header value
    ///   characters.
    #[inline]
    fn insert_ascii(
        &mut self,
        key: impl HeaderKeyExtT,
        value: impl TryInto<HeaderValue, Error = InvalidHeaderValue>,
    ) -> Result<&mut Self, InvalidHeaderValue> {
        self.insert_exact(key.as_header_name(), value.try_into()?);
        Ok(self)
    }

    /// Insert general http header, for any value that implements [`StringExtT`]
    ///
    /// For gRPC metadata, please instead use [`MetadataMapExtT::insert_bin`] or
    /// [`MetadataMapExtT::insert_bin_byte`].
    ///
    /// Key **SHOULD NOT** be a binary type gRPC Metadata key, though for
    /// performance consideration, we will not check so.
    ///
    /// # Error
    ///
    /// - [`InvalidHeaderValue`] if the value contains invalid header value
    ///   characters.
    #[inline]
    fn insert_ascii_any(
        &mut self,
        key: impl HeaderKeyExtT,
        value: impl StringExtT,
    ) -> Result<&mut Self, InvalidHeaderValue> {
        self.insert_exact(key.as_header_name(), value.to_http_header_value()?);
        Ok(self)
    }

    /// Insert general http header
    ///
    /// For gRPC metadata, please instead use [`MetadataMapExtT::insert_bin`] or
    /// [`MetadataMapExtT::insert_bin_byte`].
    ///
    /// Key **SHOULD NOT** be a binary type gRPC Metadata key, though for
    /// performance consideration, we will not check so.
    #[inline]
    fn insert_ascii_infallible(
        &mut self,
        key: impl HeaderKeyExtT,
        value: impl TryInto<HeaderValue, Error = std::convert::Infallible>,
    ) -> &mut Self {
        self.insert_exact(key.as_header_name(), value.try_into().unwrap());
        self
    }

    /// Insert general http header from &'static str
    ///
    /// # Panic
    ///
    /// - The argument `value` contains invalid header value characters.
    #[inline]
    fn insert_static(&mut self, key: impl HeaderKeyExtT, value: &'static str) -> &mut Self {
        self.insert_exact(key.as_header_name(), HeaderValue::from_static(value));
        self
    }

    /// Insert default value of `T` that implement [`HeaderKeyExtT`]
    ///
    /// It's a no-op if there's no default value.
    #[inline]
    fn insert_default(&mut self, key: impl HeaderKeyExtT) -> &mut Self {
        if let Some(v) = key.default_header_value() {
            self.insert_exact(key.as_header_name(), v);
        }
        self
    }

    /// Check if key exist, just a bridge to [`HeaderMap`] or any else
    fn contains_headerkey(&self, key: impl HeaderKeyExtT) -> bool;

    /// Get value with exact type, just a bridge to [`HeaderMap`] or any else
    ///
    /// Accept any key type that can be converted to [`HeaderName`], see
    /// [`AsHeaderName`]. It can be [`HeaderName`], &'a [`HeaderName`], &'a
    /// [str] or [String].
    fn get_exact<K>(&self, key: K) -> Option<&HeaderValue>
    where
        K: AsHeaderName;

    /// Insert value with exact type, just a bridge to [`HeaderMap`] or any else
    fn insert_exact(&mut self, key: HeaderName, value: HeaderValue) -> &mut Self;
}

// auto impl for `&mut T`
impl<T> HeaderMapExtT for &mut T
where
    T: HeaderMapExtT,
{
    #[inline]
    fn contains_headerkey(&self, key: impl HeaderKeyExtT) -> bool {
        (**self).contains_headerkey(key)
    }

    #[inline]
    fn get_exact<K>(&self, key: K) -> Option<&HeaderValue>
    where
        K: AsHeaderName,
    {
        (**self).get_exact(key)
    }

    #[inline]
    fn insert_exact(&mut self, key: HeaderName, value: HeaderValue) -> &mut Self {
        (**self).insert_exact(key, value);
        self
    }
}

/// Trait for extending [`http::HeaderMap`]'s methods, for gRPC Metadata.
///
/// If `T` implements this trait, `&mut T` will also implement this trait.
pub trait MetadataMapExtT: HeaderMapExtT {
    #[inline]
    /// Get gRPC Binary type Metadata
    ///
    /// # Error
    ///
    /// - Invalid Base64 string.
    ///
    /// # Panic
    ///
    /// - the argument `key` is not a valid binary type gRPC Metadata key.
    fn get_bin(&self, key: impl HeaderKeyExtT) -> Result<Option<Vec<u8>>> {
        debug_assert!(
            key.is_binary_key(),
            "[{}] is not a valid binary type gRPC Metadata key",
            key.as_str()
        );

        if let Some(b64_str) = self.get_ascii(key) {
            let decoded_bytes = b64_decode!(b64_str, STANDARD_NO_PAD)
                .map_err(|e| anyhow!("Invalid base64 string: [{b64_str}]").context(e))?;
            Ok(Some(decoded_bytes))
        } else {
            Ok(None)
        }
    }

    #[inline]
    /// Get gRPC Binary type Metadata, decoded as target struct.
    ///
    /// # Error
    ///
    /// - [`prost::DecodeError`].
    /// - Invalid Base64 string.
    ///
    /// # Panic
    ///
    /// - the argument `key` is not a valid binary type gRPC Metadata key.
    fn get_bin_decoded<T>(&self, key: impl HeaderKeyExtT) -> Result<Option<T>>
    where
        T: prost::Message + Default,
    {
        if let Some(bin) = self.get_bin(key)? {
            Ok(Some(T::decode(bin.as_slice())?))
        } else {
            Ok(None)
        }
    }

    #[inline]
    /// Insert gRPC Binary type Metadata from **base64 encoded** str.
    ///
    /// If need to insert raw binary data, please instead use
    /// [`MetadataMapExtT::insert_bin_byte`]. If need to insert struct data,
    /// please instead use [`MetadataMapExtT::insert_bin_struct`].
    ///
    /// # Error
    ///
    /// - [`InvalidHeaderValue`] if the value contains invalid header value
    ///   characters.
    ///
    /// # Panic
    ///
    /// - the argument `key` is not a valid binary type gRPC Metadata key.
    fn insert_bin(
        &mut self,
        key: impl HeaderKeyExtT,
        value: impl TryInto<HeaderValue, Error = InvalidHeaderValue>,
    ) -> Result<&mut Self, InvalidHeaderValue> {
        debug_assert!(
            key.is_binary_key(),
            "[{}] is not a valid binary type gRPC Metadata key",
            key.as_str()
        );

        self.insert_exact(key.as_header_name(), value.try_into()?);
        Ok(self)
    }

    #[inline]
    /// Insert gRPC Binary type Metadata from **raw binary data**
    ///
    /// Input value should not be base64 encoded str byte but raw binary data,
    /// or please instead use `insert_bin`.
    ///
    /// # Panic
    ///
    /// - the argument `key` is not a valid binary type gRPC Metadata key.
    fn insert_bin_byte(&mut self, key: impl HeaderKeyExtT, value: impl AsRef<[u8]>) -> &mut Self {
        debug_assert!(
            key.is_binary_key(),
            "[{}] is not a valid binary type gRPC Metadata key",
            key.as_str()
        );

        // SAFE: Base64 encoded data value must be valid http header value
        // Here we avoid copy_from_slice since we own the data
        let value = HeaderValue::from_maybe_shared(b64_encode_bytes!(value)).unwrap();
        self.insert_exact(key.as_header_name(), value);

        self
    }

    /// Insert gRPC Binary type Metadata from struct
    ///
    /// # Panic
    ///
    /// - the argument `key` is not a valid binary type gRPC Metadata key.
    fn insert_bin_struct<T>(&mut self, key: impl HeaderKeyExtT, value: T) -> &mut Self
    where
        T: prost::Message,
    {
        debug_assert!(
            key.is_binary_key(),
            "[{}] is not a valid binary type gRPC Metadata key",
            key.as_str()
        );

        let mut buf = bytes::BytesMut::with_capacity(64);
        let _ = value.encode(&mut buf);

        self.insert_exact(
            key.as_header_name(),
            HeaderValue::from_maybe_shared(buf).unwrap(),
        );

        self
    }
}

// auto impl for `&mut T`
impl<T> MetadataMapExtT for &mut T where T: MetadataMapExtT {}

impl HeaderMapExtT for HeaderMap {
    #[inline]
    fn contains_headerkey(&self, key: impl HeaderKeyExtT) -> bool {
        self.contains_key(key.as_header_name())
    }

    #[inline]
    fn get_exact<K>(&self, key: K) -> Option<&HeaderValue>
    where
        K: AsHeaderName,
    {
        self.get(key)
    }

    #[inline]
    fn insert_exact(&mut self, key: HeaderName, value: HeaderValue) -> &mut Self {
        self.insert(key, value);
        self
    }
}

impl MetadataMapExtT for HeaderMap {}
