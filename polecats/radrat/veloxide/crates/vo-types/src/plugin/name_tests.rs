#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod name_tests {
    use crate::plugin::PluginName;
    use crate::plugin::PLUGIN_NAME_MAX_LEN;
    use crate::ParseError;

    #[test]
    fn plugin_name_accepts_valid_alphanumeric_with_hyphens() {
        let name = PluginName::new("merge-resolver").expect("valid plugin name");
        assert_eq!(name.as_str(), "merge-resolver");
    }

    #[test]
    fn plugin_name_accepts_single_char() {
        let name = PluginName::new("a").expect("single char is valid");
        assert_eq!(name.as_str(), "a");
    }

    #[test]
    fn plugin_name_accepts_max_length() {
        let input = "a".repeat(PLUGIN_NAME_MAX_LEN);
        let name = PluginName::new(&input).expect("max length is valid");
        assert_eq!(name.as_str(), input);
    }

    #[test]
    fn plugin_name_rejects_empty_with_empty_error() {
        let result = PluginName::new("");
        assert_eq!(
            result,
            Err(ParseError::Empty {
                type_name: "PluginName"
            })
        );
    }

    #[test]
    fn plugin_name_rejects_over_max_length_with_exceeds_max_length_error() {
        let input = "a".repeat(PLUGIN_NAME_MAX_LEN + 1);
        let result = PluginName::new(&input);
        assert!(matches!(
            result,
            Err(ParseError::ExceedsMaxLength {
                type_name: "PluginName",
                actual: 65,
                ..
            })
        ));
    }

    #[test]
    fn plugin_name_rejects_underscore_with_invalid_characters_error() {
        let result = PluginName::new("merge_resolver");
        assert!(matches!(
            result,
            Err(ParseError::InvalidCharacters {
                type_name: "PluginName",
                ..
            })
        ));
    }

    #[test]
    fn plugin_name_rejects_spaces_with_invalid_characters_error() {
        let result = PluginName::new("merge resolver");
        assert!(matches!(
            result,
            Err(ParseError::InvalidCharacters {
                type_name: "PluginName",
                ..
            })
        ));
    }

    #[test]
    fn plugin_name_rejects_special_chars_with_invalid_characters_error() {
        let result = PluginName::new("merge@resolver!");
        assert!(matches!(
            result,
            Err(ParseError::InvalidCharacters {
                type_name: "PluginName",
                ..
            })
        ));
    }

    #[test]
    fn plugin_name_display_returns_raw_string() {
        let name = PluginName::new("blob-connector").unwrap();
        assert_eq!(format!("{name}"), "blob-connector");
    }

    #[test]
    fn plugin_name_accepts_all_numeric() {
        let name = PluginName::new("12345").expect("all numeric is valid");
        assert_eq!(name.as_str(), "12345");
    }

    #[test]
    fn plugin_name_accepts_leading_digit() {
        let name = PluginName::new("2fa-handler").expect("leading digit is valid");
        assert_eq!(name.as_str(), "2fa-handler");
    }

    #[test]
    fn plugin_name_rejects_empty_after_whitespace_trim() {
        let result = PluginName::new("   ");
        assert!(result.is_err());
    }
}
