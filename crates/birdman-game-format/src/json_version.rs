//! Cold text and version admission shared by independent JSON source contracts.

use serde::Deserialize;

/// Borrows UTF-8 JSON for parsing while retaining the original bytes for identity and storage.
pub(crate) fn source_text(input: &[u8]) -> Result<&str, core::str::Utf8Error> {
    let text = core::str::from_utf8(input)?;
    Ok(text.strip_prefix('\u{feff}').unwrap_or(text))
}

pub(crate) fn declared_schema_version(input: &str) -> Result<u32, serde_json::Error> {
    use serde::de::{Error, IgnoredAny, MapAccess, Visitor};

    #[derive(Deserialize)]
    #[serde(field_identifier, rename_all = "snake_case")]
    enum Field {
        SchemaVersion,
        #[serde(other)]
        Other,
    }
    struct VersionVisitor;
    impl<'de> Visitor<'de> for VersionVisitor {
        type Value = u32;

        fn expecting(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            formatter.write_str("a JSON object with schema_version")
        }

        fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<u32, M::Error> {
            let mut version = None;
            while let Some(field) = map.next_key()? {
                match field {
                    Field::SchemaVersion => {
                        if version.is_some() {
                            return Err(M::Error::duplicate_field("schema_version"));
                        }
                        version = Some(map.next_value::<u32>()?);
                    }
                    Field::Other => {
                        map.next_value::<IgnoredAny>()?;
                    }
                }
            }
            version.ok_or_else(|| M::Error::missing_field("schema_version"))
        }
    }
    let mut decoder = serde_json::Deserializer::from_str(input);
    let version = serde::Deserializer::deserialize_map(&mut decoder, VersionVisitor)?;
    decoder.end()?;
    Ok(version)
}
