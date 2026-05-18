// SPDX-License-Identifier: MIT
// Copyright (c) 2026 AppThere

//! Serialization, deserialization, `Display`, and `FromStr` for [`super::FileAccessToken`].

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use std::path::PathBuf;

use crate::error::TokenParseError;
use super::{FileAccessToken, TokenInner};

impl FileAccessToken {
    /// Serialize the token to a URL-safe base64-encoded string for storage.
    #[must_use]
    pub fn serialize(&self) -> String {
        // Serialization of the inner enum to JSON should not fail for our
        // data types (no maps with non-string keys, no infinite floats).
        // However, we handle the error path gracefully by returning an
        // empty-object JSON fallback, which will fail on deserialization
        // with a clear error rather than panicking here.
        let json = match serde_json::to_string(&self.inner) {
            Ok(j) => j,
            Err(_) => return URL_SAFE_NO_PAD.encode(b"{}"),
        };
        URL_SAFE_NO_PAD.encode(json.as_bytes())
    }

    /// Deserialize a token from a string previously returned by [`serialize`](Self::serialize).
    ///
    /// # Errors
    ///
    /// Returns [`TokenParseError`] if the string is malformed.
    pub fn deserialize(s: &str) -> Result<Self, TokenParseError> {
        let bytes = URL_SAFE_NO_PAD
            .decode(s)
            .map_err(|e| TokenParseError::InvalidBase64 {
                message: e.to_string(),
            })?;

        let json = String::from_utf8(bytes).map_err(|e| TokenParseError::InvalidBase64 {
            message: e.to_string(),
        })?;

        let inner: TokenInner =
            serde_json::from_str(&json).map_err(|e| TokenParseError::InvalidJson {
                message: e.to_string(),
            })?;

        Ok(Self { inner })
    }
}

impl std::fmt::Display for FileAccessToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.serialize())
    }
}

impl std::str::FromStr for FileAccessToken {
    type Err = TokenParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::deserialize(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_desktop_token() {
        let token = FileAccessToken {
            inner: TokenInner::Desktop {
                path: PathBuf::from("/tmp/test.txt"),
                display_name: "test.txt".into(),
            },
        };
        let serialized = token.serialize();
        let restored = FileAccessToken::deserialize(&serialized).unwrap();
        assert_eq!(restored.display_name(), "test.txt");
        assert!(restored.mime_type().is_none());
    }

    #[test]
    fn round_trip_android_token() {
        let token = FileAccessToken {
            inner: TokenInner::Android {
                uri: "content://com.example/doc/1".into(),
                display_name: "photo.jpg".into(),
                mime_type: Some("image/jpeg".into()),
            },
        };
        let serialized = token.serialize();
        let restored = FileAccessToken::deserialize(&serialized).unwrap();
        assert_eq!(restored.display_name(), "photo.jpg");
        assert_eq!(restored.mime_type(), Some("image/jpeg"));
    }

    #[test]
    fn round_trip_ios_token() {
        let token = FileAccessToken {
            inner: TokenInner::Ios {
                bookmark: vec![0xDE, 0xAD, 0xBE, 0xEF],
                display_name: "notes.pdf".into(),
                mime_type: Some("application/pdf".into()),
            },
        };
        let serialized = token.serialize();
        let restored = FileAccessToken::deserialize(&serialized).unwrap();
        assert_eq!(restored.display_name(), "notes.pdf");
        assert_eq!(restored.mime_type(), Some("application/pdf"));
    }

    #[test]
    fn round_trip_wasm_token() {
        let token = FileAccessToken {
            inner: TokenInner::Wasm {
                data: vec![1, 2, 3, 4, 5],
                name: "data.bin".into(),
                mime_type: Some("application/octet-stream".into()),
            },
        };
        let serialized = token.serialize();
        let restored = FileAccessToken::deserialize(&serialized).unwrap();
        assert_eq!(restored.display_name(), "data.bin");
        assert_eq!(restored.mime_type(), Some("application/octet-stream"));
    }

    #[test]
    fn deserialize_invalid_base64_returns_error() {
        let result = FileAccessToken::deserialize("not!valid!base64!!!");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TokenParseError::InvalidBase64 { .. }
        ));
    }

    #[test]
    fn deserialize_invalid_json_returns_error() {
        let bad = URL_SAFE_NO_PAD.encode(b"not json");
        let result = FileAccessToken::deserialize(&bad);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TokenParseError::InvalidJson { .. }
        ));
    }

    #[test]
    fn display_and_from_str_round_trip() {
        let token = FileAccessToken {
            inner: TokenInner::Desktop {
                path: PathBuf::from("/tmp/x.txt"),
                display_name: "x.txt".into(),
            },
        };
        let s = token.to_string();
        let restored: FileAccessToken = s.parse().unwrap();
        assert_eq!(restored.display_name(), "x.txt");
    }
}
