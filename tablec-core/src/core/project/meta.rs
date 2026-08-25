use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct ToolVersion {
    pub tablec: String,
    pub calamine: &'static str,
    pub serde_json: &'static str,
    pub blake3: &'static str,
}

impl Default for ToolVersion {
    fn default() -> Self {
        Self {
            tablec: env!("CARGO_PKG_VERSION").to_string(),
            calamine: "0.25.0",
            serde_json: "1.0.122",
            blake3: "1",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Meta {
    pub version: String,
    pub hash: [u8; 32],
    pub build_at: i64,
    pub source: Vec<PathBuf>,
    pub tool: ToolVersion,
}

impl Default for Meta {
    fn default() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            hash: [0u8; 32],
            build_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
            source: Vec::new(),
            tool: ToolVersion::default(),
        }
    }
}

impl Meta {
    pub fn hash_hex(&self) -> String {
        self.hash.iter().map(|b| format!("{:02x}", b)).collect()
    }
}

impl Serialize for Meta {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        // Use serialize_map so binary formats (msgpack) produce a map,
        // which roundtrips with our visit_map-based Deserialize.
        let mut map = s.serialize_map(Some(5))?;
        map.serialize_entry("version", &self.version)?;
        map.serialize_entry("hash", &self.hash_hex())?;
        map.serialize_entry("build_at", &self.build_at)?;
        map.serialize_entry(
            "source",
            &self
                .source
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>(),
        )?;
        map.serialize_entry("tool", &self.tool)?;
        map.end()
    }
}

impl<'de> Deserialize<'de> for Meta {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::{MapAccess, Visitor};
        use std::fmt;

        // Custom visitor reading exactly the 5 fields we serialize.
        // Works for both JSON and binary (msgpack) because it accepts
        // an arbitrary map deserializer.
        struct MetaVisitor;
        impl<'de> Visitor<'de> for MetaVisitor {
            type Value = Meta;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("struct Meta")
            }
            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Meta, M::Error> {
                let mut version: Option<String> = None;
                let mut hash: Option<String> = None;
                let mut build_at: Option<i64> = None;
                let mut source: Option<Vec<String>> = None;
                let mut tool: Option<Option<ToolVersion>> = None;
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "version" => version = Some(map.next_value()?),
                        "hash" => hash = Some(map.next_value()?),
                        "build_at" => build_at = Some(map.next_value()?),
                        "source" => source = Some(map.next_value()?),
                        "tool" => tool = Some(map.next_value()?),
                        _ => {
                            let _: serde::de::IgnoredAny = map.next_value()?;
                        }
                    }
                }
                let version = version.ok_or_else(|| serde::de::Error::missing_field("version"))?;
                let hash_s = hash.ok_or_else(|| serde::de::Error::missing_field("hash"))?;
                let build_at =
                    build_at.ok_or_else(|| serde::de::Error::missing_field("build_at"))?;
                let source = source.unwrap_or_default();
                let tool = tool.flatten().unwrap_or_default();
                if hash_s.len() != 64 {
                    return Err(serde::de::Error::custom("hash must be 64-char hex"));
                }
                let mut hash_b = [0u8; 32];
                for i in 0..32 {
                    hash_b[i] = u8::from_str_radix(&hash_s[i * 2..i * 2 + 2], 16)
                        .map_err(serde::de::Error::custom)?;
                }
                Ok(Meta {
                    version,
                    hash: hash_b,
                    build_at,
                    source: source.into_iter().map(PathBuf::from).collect(),
                    tool,
                })
            }
        }

        d.deserialize_struct(
            "Meta",
            &["version", "hash", "build_at", "source", "tool"],
            MetaVisitor,
        )
    }
}

impl Serialize for ToolVersion {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        // serialize_map for binary format compatibility.
        let mut map = s.serialize_map(Some(4))?;
        map.serialize_entry("tablec", &self.tablec)?;
        map.serialize_entry("calamine", self.calamine)?;
        map.serialize_entry("serde_json", self.serde_json)?;
        map.serialize_entry("blake3", self.blake3)?;
        map.end()
    }
}

impl<'de> Deserialize<'de> for ToolVersion {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::{MapAccess, Visitor};
        use std::fmt;

        struct ToolVersionVisitor;
        impl<'de> Visitor<'de> for ToolVersionVisitor {
            type Value = ToolVersion;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("struct ToolVersion")
            }
            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<ToolVersion, M::Error> {
                let mut tablec: Option<String> = None;
                let mut calamine: Option<String> = None;
                let mut serde_json: Option<String> = None;
                let mut blake3: Option<String> = None;
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "tablec" => tablec = Some(map.next_value()?),
                        "calamine" => calamine = Some(map.next_value()?),
                        "serde_json" => serde_json = Some(map.next_value()?),
                        "blake3" => blake3 = Some(map.next_value()?),
                        _ => {
                            let _: serde::de::IgnoredAny = map.next_value()?;
                        }
                    }
                }
                let tablec = tablec.ok_or_else(|| serde::de::Error::missing_field("tablec"))?;
                let calamine = calamine.unwrap_or_else(|| "0.25.0".to_string());
                let serde_json = serde_json.unwrap_or_else(|| "1.0.122".to_string());
                let blake3 = blake3.unwrap_or_else(|| "1".to_string());
                // Convert owned Strings to &'static str. Acceptable here because
                // version strings are tiny and bounded; roundtrip only.
                Ok(ToolVersion {
                    tablec,
                    calamine: Box::leak(calamine.into_boxed_str()),
                    serde_json: Box::leak(serde_json.into_boxed_str()),
                    blake3: Box::leak(blake3.into_boxed_str()),
                })
            }
        }

        d.deserialize_struct(
            "ToolVersion",
            &["tablec", "calamine", "serde_json", "blake3"],
            ToolVersionVisitor,
        )
    }
}

impl std::fmt::Display for Meta {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "hash={} version={} build_at={} source={:?} tool=tablec/{}",
            self.hash_hex(),
            self.version,
            self.build_at,
            self.source,
            self.tool.tablec
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_hex_is_64_chars() {
        let meta = Meta::default();
        assert_eq!(meta.hash_hex().len(), 64);
    }

    #[test]
    fn hash_hex_roundtrip_through_json() {
        let mut meta = Meta::default();
        meta.hash = [42u8; 32];
        let json = serde_json::to_string(&meta).unwrap();
        let meta2: Meta = serde_json::from_str(&json).unwrap();
        assert_eq!(meta.hash, meta2.hash);
    }

    #[test]
    fn json_hash_field_is_string_not_array() {
        let mut meta = Meta::default();
        meta.hash = [1u8; 32];
        let json = serde_json::to_string(&meta).unwrap();
        assert!(
            json.contains("\"hash\":\"01010101"),
            "hash must serialize as hex string, got: {}",
            json
        );
    }

    // Brief Task 2: `Meta` has no `parse(...)` method — deserialization goes
    // through the `serde::Deserialize` impl, so "parse" tests below use
    // `serde_json::from_str` directly.

    const ZERO_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";

    #[test]
    fn parse_minimal_with_required_fields_returns_ok() {
        // version, hash, build_at are required; source/tool default.
        let json = format!(r#"{{"version":"1.0.0","hash":"{ZERO_HASH}","build_at":1}}"#);
        let meta: Meta = serde_json::from_str(&json).unwrap();
        assert_eq!(meta.version, "1.0.0");
        assert_eq!(meta.hash, [0u8; 32]);
        assert_eq!(meta.build_at, 1);
        assert!(meta.source.is_empty(), "source defaults to empty Vec");
        // tool defaults applied.
        assert_eq!(meta.tool.calamine, "0.25.0");
    }

    #[test]
    fn parse_missing_version_returns_err() {
        let json = format!(r#"{{"hash":"{ZERO_HASH}","build_at":1}}"#);
        let err = serde_json::from_str::<Meta>(&json).unwrap_err().to_string();
        assert!(
            err.contains("version"),
            "error must point at missing 'version', got: {err}"
        );
    }

    #[test]
    fn parse_missing_hash_returns_err() {
        let json = r#"{"version":"1.0.0","build_at":1}"#;
        let err = serde_json::from_str::<Meta>(json).unwrap_err().to_string();
        assert!(
            err.contains("hash"),
            "error must point at missing 'hash', got: {err}"
        );
    }

    #[test]
    fn parse_missing_build_at_returns_err() {
        let json = format!(r#"{{"version":"1.0.0","hash":"{ZERO_HASH}"}}"#);
        let err = serde_json::from_str::<Meta>(&json).unwrap_err().to_string();
        assert!(
            err.contains("build_at"),
            "error must point at missing 'build_at', got: {err}"
        );
    }

    #[test]
    fn parse_invalid_json_returns_err() {
        let json = "{ this is: not: valid json";
        assert!(
            serde_json::from_str::<Meta>(json).is_err(),
            "malformed input must fail to deserialize"
        );
    }

    #[test]
    fn parse_hash_wrong_length_returns_err() {
        // 63 chars instead of 64 -> fail length check.
        let short = "0".repeat(63);
        let json = format!(r#"{{"version":"1.0.0","hash":"{short}","build_at":1}}"#);
        let err = serde_json::from_str::<Meta>(&json).unwrap_err().to_string();
        assert!(
            err.contains("64"),
            "error must mention the required 64-char length, got: {err}"
        );
    }

    #[test]
    fn parse_hash_invalid_hex_chars_returns_err() {
        // 64 chars but 'z' is not a valid hex digit.
        let bad = "z".repeat(64);
        let json = format!(r#"{{"version":"1.0.0","hash":"{bad}","build_at":1}}"#);
        assert!(
            serde_json::from_str::<Meta>(&json).is_err(),
            "non-hex hash chars must fail to deserialize"
        );
    }

    #[test]
    fn full_roundtrip_preserves_all_fields() {
        let mut meta = Meta::default();
        meta.hash = [0xab; 32];
        meta.build_at = 1_700_000_000;
        meta.source = vec![PathBuf::from("/tmp/a"), PathBuf::from("/tmp/b")];
        let json = serde_json::to_string(&meta).unwrap();
        let meta2: Meta = serde_json::from_str(&json).unwrap();
        assert_eq!(meta.version, meta2.version);
        assert_eq!(meta.hash, meta2.hash);
        assert_eq!(meta.build_at, meta2.build_at);
        assert_eq!(meta.source, meta2.source);
        assert_eq!(meta.tool.tablec, meta2.tool.tablec);
        assert_eq!(meta.tool.calamine, meta2.tool.calamine);
        assert_eq!(meta.tool.serde_json, meta2.tool.serde_json);
        assert_eq!(meta.tool.blake3, meta2.tool.blake3);
    }

    #[test]
    fn parse_extra_unknown_fields_are_ignored() {
        let json =
            format!(r#"{{"version":"1.0.0","hash":"{ZERO_HASH}","build_at":1,"future_field":42}}"#);
        let meta: Meta = serde_json::from_str(&json).unwrap();
        assert_eq!(meta.version, "1.0.0");
    }
}
