use super::error::ProductError;

const PREFIX: &str = "generator:v1:";
const MAX_ENCODED_BYTES: usize = 8192;

/// A transport locator, not an authorization token. Every mutation still goes
/// through the binding authority's project and exact-pair validation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExactGeneratorSelection {
    pub workflow_version_id: String,
    pub recipe_id: String,
}

impl ExactGeneratorSelection {
    pub fn encode(&self) -> Result<String, ProductError> {
        if self.workflow_version_id.trim().is_empty() || self.recipe_id.trim().is_empty() {
            return Err(ProductError::invalid_selection());
        }
        let bytes = serde_json::to_vec(&[&self.workflow_version_id, &self.recipe_id])
            .map_err(|_| ProductError::invalid_selection())?;
        if bytes.len() > MAX_ENCODED_BYTES / 2 {
            return Err(ProductError::invalid_selection());
        }
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut encoded = String::with_capacity(PREFIX.len() + bytes.len() * 2);
        encoded.push_str(PREFIX);
        for byte in bytes {
            encoded.push(HEX[(byte >> 4) as usize] as char);
            encoded.push(HEX[(byte & 15) as usize] as char);
        }
        Ok(encoded)
    }

    pub fn decode(value: &str) -> Result<Self, ProductError> {
        let encoded = value
            .strip_prefix(PREFIX)
            .ok_or_else(ProductError::invalid_selection)?;
        if encoded.is_empty() || encoded.len() > MAX_ENCODED_BYTES || encoded.len() % 2 != 0 {
            return Err(ProductError::invalid_selection());
        }
        fn nibble(byte: u8) -> Result<u8, ProductError> {
            match byte {
                b'0'..=b'9' => Ok(byte - b'0'),
                b'a'..=b'f' => Ok(byte - b'a' + 10),
                _ => Err(ProductError::invalid_selection()),
            }
        }
        let bytes = encoded
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| Ok((nibble(pair[0])? << 4) | nibble(pair[1])?))
            .collect::<Result<Vec<_>, ProductError>>()?;
        let [workflow_version_id, recipe_id]: [String; 2] =
            serde_json::from_slice(&bytes).map_err(|_| ProductError::invalid_selection())?;
        let selection = Self {
            workflow_version_id,
            recipe_id,
        };
        // Reject noncanonical representations, including whitespace/escape aliases.
        if selection.encode()? != value {
            return Err(ProductError::invalid_selection());
        }
        Ok(selection)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn product_selection_ref_exact_roundtrip() {
        let original = ExactGeneratorSelection {
            workflow_version_id: "wfv:旧版本/🖼".to_owned(),
            recipe_id: "recipe:准确配方".to_owned(),
        };
        let encoded = original.encode().unwrap();
        assert_eq!(ExactGeneratorSelection::decode(&encoded).unwrap(), original);
        assert!(!encoded.contains(&original.workflow_version_id));
        assert!(!encoded.contains(&original.recipe_id));
    }

    #[test]
    fn product_selection_ref_rejects_malformed_or_unknown_versions() {
        for value in [
            "",
            "generator:v2:00",
            "generator:v1:0",
            "generator:v1:gg",
            "generator:v1:ff",
            "generator:v1:5b5d",
        ] {
            let error = ExactGeneratorSelection::decode(value).unwrap_err();
            assert_eq!(error.code, "GENERATOR_UNAVAILABLE");
            assert!(!error.details.retryable);
        }
        assert!(ExactGeneratorSelection::decode(&format!("{PREFIX}{}", "0".repeat(8194))).is_err());
    }
}
