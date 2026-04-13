#![cfg(feature = "proptest")]

use proptest::prelude::*;
use proptest::string::string_regex;

use crate::plugin::*;
use crate::FenceToken;

proptest::proptest! {
    #[test]
    fn plugin_version_ordering_is_transitive(
        v1_major in 0u32..10u32,
        v1_minor in 0u32..10u32,
        v1_patch in 0u32..10u32,
        v2_major in 0u32..10u32,
        v2_minor in 0u32..10u32,
        v2_patch in 0u32..10u32,
        v3_major in 0u32..10u32,
        v3_minor in 0u32..10u32,
        v3_patch in 0u32..10u32,
    ) {
        let v1 = PluginVersion::new(v1_major, v1_minor, v1_patch);
        let v2 = PluginVersion::new(v2_major, v2_minor, v2_patch);
        let v3 = PluginVersion::new(v3_major, v3_minor, v3_patch);

        if v1 < v2 && v2 < v3 {
            prop_assert!(v1 < v3, "transitivity violated: {:?} < {:?} < {:?} but {:?} >= {:?}", v1, v2, v3, v1, v3);
        }
    }

    #[test]
    fn plugin_version_compatibility_is_reflexive(
        major in 0u32..100u32,
        minor in 0u32..100u32,
        patch in 0u32..100u32,
    ) {
        let v = PluginVersion::new(major, minor, patch);
        prop_assert!(v.is_compatible_with(&v));
    }

    #[test]
    fn plugin_version_compatibility_is_symmetric(
        major1 in 0u32..100u32,
        minor1 in 0u32..100u32,
        patch1 in 0u32..100u32,
        major2 in 0u32..100u32,
        minor2 in 0u32..100u32,
        patch2 in 0u32..100u32,
    ) {
        let v1 = PluginVersion::new(major1, minor1, patch1);
        let v2 = PluginVersion::new(major2, minor2, patch2);
        prop_assert_eq!(v1.is_compatible_with(&v2), v2.is_compatible_with(&v1));
    }

    #[test]
    fn fence_token_next_is_strictly_increasing(value in 1u64..u64::MAX) {
        let token = FenceToken::new(value).unwrap();
        let next = token.next().unwrap();
        prop_assert!(next > token);
    }

    #[test]
    fn plugin_name_rejects_empty() {
        prop_assert!(PluginName::new("").is_err());
    }

    #[test]
    fn plugin_name_rejects_over_max_length(s in 0usize..200usize) {
        let input = "a".repeat(s);
        if s == 0 {
            prop_assert!(PluginName::new(&input).is_err());
        } else if s > PLUGIN_NAME_MAX_LEN {
            prop_assert!(PluginName::new(&input).is_err());
        }
    }
}
