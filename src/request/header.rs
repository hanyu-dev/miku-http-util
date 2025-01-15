//! HTTP request utilities: HTTP header related.

use std::convert::Infallible;

use anyhow::{anyhow, Result};
use http::{
    header::{AsHeaderName, InvalidHeaderValue},
    HeaderMap, HeaderName, HeaderValue,
};
use macro_toolset::{
    b64_decode, b64_encode,
    string::{base64::Base64EncoderT, StringExtT},
    wrapper,
};

/// Trait helper for managing HTTP header keys.
pub trait HeaderKeyT {
    /// `as_str_ext` and most times should be &'static
    fn as_str_ext(&self) -> &str;

    /// Get the key name
    fn to_header_name(self) -> HeaderName;

    /// Get default value of the key
    ///
    /// Return `None` if no default one.
    fn default_header_value(&self) -> Option<HeaderValue> {
        None
    }
}

/// Trait helper for managing HTTP header keys.
///
/// Marker trait for binary keys.
pub trait HeaderAsciiKeyT: HeaderKeyT {}

/// Trait helper for managing HTTP header keys.
///
/// Marker trait for binary keys.
pub trait HeaderBinaryKeyT: HeaderKeyT {}

impl HeaderKeyT for &'static str {
    #[inline]
    fn as_str_ext(&self) -> &str {
        self
    }

    #[inline]
    fn to_header_name(self) -> HeaderName {
        HeaderName::from_static(self)
    }
}

impl HeaderAsciiKeyT for &'static str {}

impl HeaderKeyT for HeaderName {
    #[inline]
    fn as_str_ext(&self) -> &str {
        self.as_str()
    }

    #[inline]
    fn to_header_name(self) -> HeaderName {
        self
    }
}

impl HeaderAsciiKeyT for HeaderName {}

wrapper! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    /// Wrapper for binary key, though you have to make sure the key is valid (with `-bin` suffix).
    ///
    /// # Panics
    ///
    /// Panics if the key is not valid (with `-bin` suffix) when debug mode.
    pub BinaryKeyWrapper<T>(pub T)
}

impl<T: HeaderKeyT> HeaderKeyT for BinaryKeyWrapper<T> {
    #[inline]
    fn as_str_ext(&self) -> &str {
        self.inner.as_str_ext()
    }

    #[inline]
    fn to_header_name(self) -> HeaderName {
        debug_assert!(self.as_str_ext().ends_with("-bin"));

        self.inner.to_header_name()
    }

    #[inline]
    fn default_header_value(&self) -> Option<HeaderValue> {
        debug_assert!(self.as_str_ext().ends_with("-bin"));

        self.inner.default_header_value()
    }
}

impl<T: HeaderKeyT> HeaderBinaryKeyT for BinaryKeyWrapper<T> {}

/// Trait for extending [`http::HeaderMap`]'s methods.
///
/// If `T` implements this trait, `&mut T` will also implement this trait.
pub trait HeaderMapExtT {
    #[inline]
    /// Returns a reference to the value associated with the key.
    ///
    /// For gRPC Metadata, please use [`get_bin`](HeaderMapExtT::get_bin)
    /// instead.
    ///
    /// Notice: if value contains invalid header value characters(non-ascii), it
    /// will be ignored and return `None`.
    fn get_ascii<K>(&self, key: K) -> Option<&str>
    where
        K: HeaderAsciiKeyT,
    {
        self.get_maybe_ascii(key)
    }

    #[doc(hidden)]
    #[inline]
    /// See [`get_ascii`](HeaderMapExtT::get_ascii) for more details.
    fn get_maybe_ascii<K>(&self, key: K) -> Option<&str>
    where
        K: HeaderKeyT,
    {
        self.get_exact(key.to_header_name()).and_then(|v| {
            v.to_str()
                .inspect_err(|e| {
                    #[cfg(feature = "feat-tracing")]
                    tracing::warn!("Invalid header value [{v:?}]: {e:?}");
                })
                .ok()
        })
    }

    #[inline]
    /// Returns the decoded base64-encoded value associated with the key, if the
    /// key-value pair exists.
    ///
    /// # Errors
    ///
    /// - Invalid Base64 string.
    fn get_bin<K>(&self, key: K) -> Result<Option<Vec<u8>>>
    where
        K: HeaderBinaryKeyT,
    {
        if let Some(b64_str) = self.get_maybe_ascii(key) {
            let decoded_bytes = b64_decode!(STANDARD_NO_PAD: b64_str)
                .map_err(|e| anyhow!(e).context(b64_str.to_string()))?;
            Ok(Some(decoded_bytes))
        } else {
            Ok(None)
        }
    }

    #[inline]
    /// Extend the given buffer with the decoded base64-encoded value associated
    /// with the key, if the key-value pair exists.
    ///
    /// # Errors
    ///
    /// - Invalid Base64 string.
    fn get_bin_to_buffer<K>(&self, key: K, buffer: &mut Vec<u8>) -> Result<()>
    where
        K: HeaderBinaryKeyT,
    {
        if let Some(b64_str) = self.get_maybe_ascii(key) {
            b64_decode!(STANDARD_NO_PAD: b64_str, buffer)?;
        }

        Ok(())
    }

    #[inline]
    /// Returns the struct decoded from the gRPC metadata binary value, if the
    /// key-value pair exists.
    ///
    /// # Errors
    ///
    /// - [`prost::DecodeError`].
    /// - Invalid Base64 string.
    fn get_bin_struct<K, T>(&self, key: K) -> Result<Option<T>>
    where
        K: HeaderBinaryKeyT,
        T: prost::Message + Default,
    {
        if let Some(bin) = self.get_bin(key)? {
            Ok(Some(T::decode(bin.as_slice())?))
        } else {
            Ok(None)
        }
    }

    #[inline]
    /// Returns the struct decoded from the gRPC metadata binary value, or a
    /// default one if the key-value pair does not exist.
    ///
    /// # Errors
    ///
    /// - [`prost::DecodeError`].
    /// - Invalid Base64 string.
    fn get_bin_struct_or_default<K, T>(&self, key: K) -> Result<T>
    where
        K: HeaderBinaryKeyT,
        T: prost::Message + Default,
    {
        if let Some(bin) = self.get_bin(key)? {
            Ok(T::decode(bin.as_slice())?)
        } else {
            Ok(T::default())
        }
    }

    /// Inserts a key-value pair into the inner [`HeaderMap`].
    ///
    /// For gRPC Metadata, please use
    /// [`insert_bin`](HeaderMapExtT::insert_bin) instead.
    ///
    /// # Errors
    ///
    /// - [`InvalidHeaderValue`] if the value contains invalid header value
    ///   characters.
    #[inline]
    fn insert_ascii<K, V>(&mut self, key: K, value: V) -> Result<&mut Self, InvalidHeaderValue>
    where
        K: HeaderAsciiKeyT,
        V: TryInto<HeaderValue, Error = InvalidHeaderValue>,
    {
        self.insert_maybe_ascii(key, value)
    }

    #[doc(hidden)]
    /// See [`insert_ascii`](HeaderMapExtT::insert_ascii).
    #[inline]
    fn insert_maybe_ascii<K, V>(
        &mut self,
        key: K,
        value: V,
    ) -> Result<&mut Self, InvalidHeaderValue>
    where
        K: HeaderKeyT,
        V: TryInto<HeaderValue, Error = InvalidHeaderValue>,
    {
        self.insert_exact(key.to_header_name(), value.try_into()?);
        Ok(self)
    }

    /// Inserts a key-value pair into the inner [`HeaderMap`].
    ///
    /// `value` can be any type that implements [`StringExtT`].
    ///
    /// For gRPC Metadata, please use
    /// [`insert_bin`](HeaderMapExtT::insert_bin) instead.
    ///
    /// # Errors
    ///
    /// - [`InvalidHeaderValue`] if the value contains invalid header value
    ///   characters.
    #[inline]
    fn insert_ascii_any<K, V>(&mut self, key: K, value: V) -> Result<&mut Self, InvalidHeaderValue>
    where
        K: HeaderAsciiKeyT,
        V: StringExtT,
    {
        self.insert_exact(key.to_header_name(), value.to_http_header_value()?);
        Ok(self)
    }

    /// Inserts a key-value pair into the inner [`HeaderMap`].
    ///
    /// For gRPC Metadata, please use
    /// [`insert_bin`](HeaderMapExtT::insert_bin) instead.
    #[inline]
    fn insert_ascii_infallible<K, V>(&mut self, key: K, value: V) -> &mut Self
    where
        K: HeaderAsciiKeyT,
        V: TryInto<HeaderValue, Error = Infallible>,
    {
        self.insert_exact(key.to_header_name(), value.try_into().unwrap());
        self
    }

    /// Inserts a key-value pair into the inner [`HeaderMap`].
    ///
    /// For gRPC Metadata, please use
    /// [`insert_bin`](HeaderMapExtT::insert_bin) instead.
    #[inline]
    fn insert_ascii_static<K>(&mut self, key: K, value: &'static str) -> &mut Self
    where
        K: HeaderAsciiKeyT,
    {
        self.insert_exact(key.to_header_name(), HeaderValue::from_static(value));
        self
    }

    /// Inserts a key-value pair into the inner [`HeaderMap`].
    ///
    /// `value` should be base64 string.
    ///
    /// # Panics
    ///
    /// Panic if the value is not a valid header value (for base64 string, it's
    /// not possible).
    #[inline]
    fn insert_bin<K, V>(&mut self, key: K, value: V) -> &mut Self
    where
        K: HeaderBinaryKeyT,
        V: TryInto<HeaderValue, Error = InvalidHeaderValue>,
    {
        self.insert_maybe_ascii(key, value)
            .expect("Base64 string should be valid header value")
    }

    /// Inserts a key-value pair into the inner [`HeaderMap`].
    ///
    /// `value` can be any type that implement [`Base64EncoderT`].
    /// See [`b64_padding::STANDARD_NO_PAD::encode`]\(data\), etc for more
    /// details.
    ///
    /// # Panics
    ///
    /// Panic if the value is not a valid header value (it's not possible unless
    /// upstream bug).
    #[inline]
    fn insert_bin_any<K, V>(&mut self, key: K, value: V) -> &mut Self
    where
        K: HeaderBinaryKeyT,
        V: Base64EncoderT,
    {
        self.insert_exact(
            key.to_header_name(),
            value
                .to_http_header_value()
                .expect("Base64 string should be valid header value"),
        )
    }

    /// Inserts a key-value pair into the inner [`HeaderMap`].
    ///
    /// `value` can be any type that implement [`AsRef`]<[u8]>.
    ///
    /// # Panics
    ///
    /// Panic if the value is not a valid header value (it's not possible unless
    /// upstream bug).
    #[inline]
    fn insert_bin_byte<K, V>(&mut self, key: K, value: V) -> &mut Self
    where
        K: HeaderBinaryKeyT,
        V: AsRef<[u8]>,
    {
        // SAFE: Base64 encoded data value must be valid http header value
        // Here we avoid copy_from_slice since we own the data
        let value = HeaderValue::from_maybe_shared(
            b64_encode!(STANDARD_NO_PAD: value.as_ref() => BYTES).freeze(),
        )
        .expect("Base64 string should be valid header value");
        self.insert_exact(key.to_header_name(), value);

        self
    }

    /// Inserts a key-value pair into the inner [`HeaderMap`].
    ///
    /// `value` can be any type that implement [`AsRef`]<[u8]>.
    ///
    /// # Errors
    ///
    /// - [`prost::EncodeError`]
    ///
    /// # Panics
    ///
    /// Panic if the value is not a valid header value (it's not possible unless
    /// upstream bug).
    #[inline]
    fn insert_bin_struct<K, V>(&mut self, key: K, value: V) -> Result<&mut Self, prost::EncodeError>
    where
        K: HeaderBinaryKeyT,
        V: prost::Message + Default,
    {
        let mut buf = Vec::with_capacity(64);
        value.encode(&mut buf)?;

        // SAFE: Base64 encoded data value must be valid http header value
        // Here we avoid copy_from_slice since we own the data
        let value =
            HeaderValue::from_maybe_shared(b64_encode!(STANDARD_NO_PAD: buf => BYTES).freeze())
                .expect("Base64 string should be valid header value");
        self.insert_exact(key.to_header_name(), value);

        Ok(self)
    }

    /// Inserts a key-value pair into the inner [`HeaderMap`].
    ///
    /// Caller must ensure the value is valid base64 string.
    #[inline]
    fn insert_bin_static<K>(&mut self, key: K, value: &'static str) -> &mut Self
    where
        K: HeaderBinaryKeyT,
    {
        self.insert_exact(key.to_header_name(), HeaderValue::from_static(value));
        self
    }

    /// Insert default value of `T` that implement [`HeaderKeyT`]
    ///
    /// It's a no-op if there's no default value.
    #[inline]
    fn insert_default(&mut self, key: impl HeaderKeyT) -> &mut Self {
        if let Some(v) = key.default_header_value() {
            self.insert_exact(key.to_header_name(), v);
        }
        self
    }

    /// Check if key exist, just a bridge to [`HeaderMap`] or any else
    fn contains_headerkey(&self, key: impl HeaderKeyT) -> bool;

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
    fn contains_headerkey(&self, key: impl HeaderKeyT) -> bool {
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

impl HeaderMapExtT for HeaderMap {
    #[inline]
    fn contains_headerkey(&self, key: impl HeaderKeyT) -> bool {
        self.contains_key(key.to_header_name())
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
