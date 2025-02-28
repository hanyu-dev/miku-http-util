//! Proxy utilities for requests.

use std::{str::FromStr, sync::Arc};

use anyhow::{anyhow, Context};
use http::HeaderValue;

const DEFAULT_SOCKS5_PROXY_PORT: u16 = 7890;

#[derive(Debug)]
#[derive(thiserror::Error)]
/// Errors for proxy utilities.
pub enum Error {
    #[error("Invalid proxy uri: {0}")]
    /// Invalid proxy uri, see [`http::uri::InvalidUri`] for more details.
    InvalidUri(#[from] http::uri::InvalidUri),

    #[error("Invalid proxy uri: unsupported scheme")]
    /// Unsupported scheme
    UnsupportedScheme,

    #[error("Invalid proxy uri: general error")]
    /// General
    General,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
/// A particular scheme used for proxying requests.
///
/// Currently only `HTTP`(s) and `SOCKS5` are supported.
///
/// # Examples
///
/// - `http://127.0.0.1:7890` // if port not specified, default to 80.
/// - `https://127.0.0.1:7890` // if port not specified, default to 443.
/// - `socks5://127.0.0.1:7890` // if port not specified, default to 7890.
/// - `socks5h://127.0.0.1:7890` // if port not specified, default to 7890.
pub enum ProxyScheme {
    /// HTTP / HTTPS proxy
    Http {
        /// is HTTPS proxy
        is_https: bool,

        /// optional HTTP Basic auth
        basic_auth: Option<HeaderValue>,

        /// proxy server's host and port
        authority: http::uri::Authority,
    },

    /// SOCKS5 proxy
    Socks5 {
        /// whether to resolve DNS remotely, aka.: "socks5" / "socks5h"
        remote_dns: bool,

        /// optional SOCKS5 auth, username and password
        password_auth: Option<(Arc<str>, Arc<str>)>,

        /// proxy server's host
        host: Arc<str>,

        /// proxy server's port
        port: u16,
    },
}

impl FromStr for ProxyScheme {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let uri = fluent_uri::Uri::parse(s).context(Error::General)?;

        let scheme = uri.scheme().as_str();
        let authority = uri.authority().ok_or(Error::General)?;
        let user_info = authority.userinfo().map(|user_info| {
            percent_encoding::percent_decode_str(user_info.as_str()).decode_utf8_lossy()
        });

        match scheme {
            "http" | "https" => {
                let authority = http::uri::Authority::try_from(format!(
                    "{}:{}",
                    authority.host(),
                    authority
                        .port_to_u16()
                        .context(Error::General)?
                        .unwrap_or_else(|| {
                            if scheme == "http" {
                                80
                            } else {
                                443
                            }
                        })
                ))
                .map_err(|e| {
                    #[cfg(debug_assertions)]
                    {
                        unreachable!("Rare bug: http::uri::Authority reports error {e:?}");
                    }

                    #[cfg(all(not(debug_assertions), feature = "feat-tracing"))]
                    {
                        tracing::error!("Rare bug: http::uri::Authority reports error {e:?}");
                    }

                    #[allow(unreachable_code)]
                    Error::InvalidUri(e)
                })?;

                let basic_auth = user_info.map(|user_info| match user_info.split_once(':') {
                    Some((user_name, password)) => basic_auth(user_name, Some(password)),
                    None => basic_auth(user_info, None::<&str>),
                });

                Ok(Self::Http {
                    is_https: scheme == "https",
                    basic_auth,
                    authority,
                })
            }
            "socks5" | "socks5h" => {
                let password_auth = match user_info {
                    Some(user_info) => Some(
                        user_info
                            .split_once(':')
                            .map(|(user_name, password)| (user_name.into(), password.into()))
                            .context("Invalid socks5 password auth")?,
                    ),
                    None => None,
                };

                Ok(Self::Socks5 {
                    remote_dns: scheme == "socks5h",
                    password_auth,
                    host: authority.host().into(),
                    port: authority
                        .port_to_u16()
                        .context(Error::General)?
                        .unwrap_or(DEFAULT_SOCKS5_PROXY_PORT),
                })
            }
            _ => {
                #[cfg(feature = "feat-tracing")]
                tracing::error!("Unsupported proxy scheme: {scheme}");
                Err(anyhow!(Error::UnsupportedScheme))
            }
        }
    }
}

impl ProxyScheme {
    /// For `HTTP` proxies, returns the optional HTTP Basic auth.
    pub const fn http_auth(&self) -> Option<&HeaderValue> {
        match self {
            ProxyScheme::Http {
                basic_auth: auth, ..
            } => auth.as_ref(),
            _ => None,
        }
    }
}

impl serde::Serialize for ProxyScheme {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            ProxyScheme::Http {
                is_https,
                basic_auth,
                authority,
            } => serializer.serialize_str(&format!(
                "{}://{}{}",
                if *is_https { "https" } else { "http" },
                basic_auth
                    .as_ref()
                    .map(|basic_auth| {
                        let basic_auth = basic_auth
                            .to_str()
                            .unwrap_or_else(|e| unreachable!("Failed to decode basic auth: {}", e))
                            .strip_prefix("Basic ")
                            .unwrap_or_else(|| {
                                unreachable!("Failed to decode basic auth: not begin with `Basic `")
                            });

                        let basic_auth = String::from_utf8(
                            base64::Engine::decode(
                                &base64::engine::general_purpose::STANDARD,
                                basic_auth,
                            )
                            .unwrap_or_else(|e| unreachable!("Failed to decode basic auth: {}", e)),
                        )
                        .unwrap_or_else(|e| unreachable!("Invalid decoded basic auth: {}", e));

                        match basic_auth.split_once(':') {
                            Some((user_name, password)) => format!(
                                "{}:{}@",
                                percent_encoding::percent_encode(
                                    user_name.as_bytes(),
                                    percent_encoding::NON_ALPHANUMERIC
                                ),
                                percent_encoding::percent_encode(
                                    password.as_bytes(),
                                    percent_encoding::NON_ALPHANUMERIC
                                )
                            ),
                            None => format!(
                                "{}@",
                                percent_encoding::percent_encode(
                                    basic_auth.as_bytes(),
                                    percent_encoding::NON_ALPHANUMERIC
                                ),
                            ),
                        }
                    })
                    .unwrap_or_default(),
                authority,
            )),
            ProxyScheme::Socks5 {
                remote_dns,
                password_auth,
                host,
                port,
            } => serializer.serialize_str(&format!(
                "{}://{}{}:{}",
                if *remote_dns { "socks5h" } else { "socks5" },
                password_auth
                    .as_ref()
                    .map(|(user_name, password)| {
                        format!(
                            "{}:{}@",
                            percent_encoding::percent_encode(
                                user_name.as_bytes(),
                                percent_encoding::NON_ALPHANUMERIC
                            ),
                            percent_encoding::percent_encode(
                                password.as_bytes(),
                                percent_encoding::NON_ALPHANUMERIC
                            )
                        )
                    })
                    .unwrap_or_default(),
                host,
                port,
            )),
        }
    }
}

impl<'de> serde::Deserialize<'de> for ProxyScheme {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        <&str>::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

fn basic_auth<U, P>(username: U, password: Option<P>) -> HeaderValue
where
    U: std::fmt::Display,
    P: std::fmt::Display,
{
    use std::io::Write;

    use base64::{prelude::BASE64_STANDARD, write::EncoderWriter};

    let mut buf = Vec::with_capacity(64);

    buf.extend(b"Basic ");

    {
        let mut encoder = EncoderWriter::new(&mut buf, &BASE64_STANDARD);
        let _ = write!(encoder, "{username}:");
        if let Some(password) = password {
            let _ = write!(encoder, "{password}");
        }
    }

    // Avoid allocation when `Bytes::from(buf)`
    buf.truncate(buf.len());

    let mut header = HeaderValue::from_maybe_shared(bytes::Bytes::from(buf))
        .expect("base64 is always valid HeaderValue");
    header.set_sensitive(true);
    header
}

#[cfg(test)]
mod tests {
    use http::HeaderValue;

    use super::*;

    #[test]
    fn test_parse_proxy_scheme() {
        assert_eq!(
            "http://127.0.0.1:7890".parse::<ProxyScheme>().unwrap(),
            ProxyScheme::Http {
                is_https: false,
                basic_auth: None,
                authority: "127.0.0.1:7890".parse().unwrap()
            }
        );
        assert_eq!(
            "http://u:p@127.0.0.1:7890".parse::<ProxyScheme>().unwrap(),
            ProxyScheme::Http {
                is_https: false,
                basic_auth: Some(HeaderValue::from_static("Basic dTpw")),
                authority: "127.0.0.1:7890".parse().unwrap() // weird but as it is
            }
        );
        assert_eq!(
            "http://u:p@127.0.0.1".parse::<ProxyScheme>().unwrap(),
            ProxyScheme::Http {
                is_https: false,
                basic_auth: Some(HeaderValue::from_static("Basic dTpw")),
                authority: "127.0.0.1:80".parse().unwrap() // weird but as it is
            }
        );
        assert_eq!(
            "https://u:p@127.0.0.1:7890".parse::<ProxyScheme>().unwrap(),
            ProxyScheme::Http {
                is_https: true,
                basic_auth: Some(HeaderValue::from_static("Basic dTpw")),
                authority: "127.0.0.1:7890".parse().unwrap() // weird but as it is
            }
        );
        assert_eq!(
            "https://u:p%40@127.0.0.1:443"
                .parse::<ProxyScheme>()
                .unwrap(),
            ProxyScheme::Http {
                is_https: true,
                basic_auth: Some(HeaderValue::from_static("Basic dTpwQA==")),
                authority: "127.0.0.1:443".parse().unwrap() // weird but as it is
            }
        );
        assert_eq!(
            "https://u:p%40@127.0.0.1".parse::<ProxyScheme>().unwrap(),
            ProxyScheme::Http {
                is_https: true,
                basic_auth: Some(HeaderValue::from_static("Basic dTpwQA==")),
                authority: "127.0.0.1:443".parse().unwrap() // weird but as it is
            }
        );
        assert_eq!(
            "socks5://u:p%40@127.0.0.1:7890"
                .parse::<ProxyScheme>()
                .unwrap(),
            ProxyScheme::Socks5 {
                remote_dns: false,
                password_auth: Some(("u".into(), "p@".into())),
                host: "127.0.0.1".into(),
                port: 7890
            }
        );
        assert_eq!(
            "socks5h://u:p%40@127.0.0.1:7890"
                .parse::<ProxyScheme>()
                .unwrap(),
            ProxyScheme::Socks5 {
                remote_dns: true,
                password_auth: Some(("u".into(), "p@".into())),
                host: "127.0.0.1".into(),
                port: DEFAULT_SOCKS5_PROXY_PORT
            }
        );
        assert_eq!(
            "socks5h://u:p%40@127.0.0.1".parse::<ProxyScheme>().unwrap(),
            ProxyScheme::Socks5 {
                remote_dns: true,
                password_auth: Some(("u".into(), "p@".into())),
                host: "127.0.0.1".into(),
                port: 7890
            }
        );
    }

    #[test]
    #[should_panic]
    fn empty_scheme() {
        "127.0.0.1:7890".parse::<ProxyScheme>().unwrap();
    }

    #[test]
    fn test_serde() {
        let scheme = ProxyScheme::Http {
            is_https: false,
            basic_auth: Some(HeaderValue::from_static("Basic dTpwQA==")),
            authority: "127.0.0.1:80".parse().unwrap(),
        };
        assert_eq!(
            serde_json::to_string(&scheme).unwrap(),
            "\"http://u:p%40@127.0.0.1:80\""
        );

        let scheme = ProxyScheme::Http {
            is_https: true,
            basic_auth: Some(HeaderValue::from_static("Basic dTpwQA==")),
            authority: "127.0.0.1:443".parse().unwrap(),
        };
        assert_eq!(
            serde_json::to_string(&scheme).unwrap(),
            "\"https://u:p%40@127.0.0.1:443\""
        );

        let scheme = ProxyScheme::Socks5 {
            remote_dns: false,
            password_auth: Some(("u".into(), "p@".into())),
            host: "127.0.0.1".into(),
            port: 7890,
        };
        assert_eq!(
            serde_json::to_string(&scheme).unwrap(),
            "\"socks5://u:p%40@127.0.0.1:7890\""
        );

        let scheme = ProxyScheme::Socks5 {
            remote_dns: true,
            password_auth: Some(("u".into(), "p@".into())),
            host: "127.0.0.1".into(),
            port: 7890,
        };
        assert_eq!(
            serde_json::to_string(&scheme).unwrap(),
            "\"socks5h://u:p%40@127.0.0.1:7890\""
        );
    }
}
