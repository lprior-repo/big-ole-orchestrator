#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod version_tests {
    use crate::plugin::PluginVersion;

    #[test]
    fn version_constructs_with_all_components() {
        let v = PluginVersion::new(2, 1, 3);
        assert_eq!(v.major(), 2);
        assert_eq!(v.minor(), 1);
        assert_eq!(v.patch(), 3);
    }

    #[test]
    fn version_zero_is_valid() {
        let v = PluginVersion::new(0, 0, 0);
        assert_eq!(v.major(), 0);
        assert_eq!(v.minor(), 0);
        assert_eq!(v.patch(), 0);
    }

    #[test]
    fn version_ordering_major_takes_precedence() {
        let v1 = PluginVersion::new(1, 9, 9);
        let v2 = PluginVersion::new(2, 0, 0);
        assert!(v1 < v2);
    }

    #[test]
    fn version_ordering_minor_breaks_major_tie() {
        let v1 = PluginVersion::new(1, 1, 9);
        let v2 = PluginVersion::new(1, 2, 0);
        assert!(v1 < v2);
    }

    #[test]
    fn version_ordering_patch_breaks_minor_tie() {
        let v1 = PluginVersion::new(1, 1, 1);
        let v2 = PluginVersion::new(1, 1, 2);
        assert!(v1 < v2);
    }

    #[test]
    fn version_ordering_equal_versions() {
        let v1 = PluginVersion::new(1, 2, 3);
        let v2 = PluginVersion::new(1, 2, 3);
        assert_eq!(v1, v2);
    }

    #[test]
    fn version_ordering_is_total() {
        let versions = [
            PluginVersion::new(0, 0, 1),
            PluginVersion::new(0, 1, 0),
            PluginVersion::new(1, 0, 0),
            PluginVersion::new(1, 0, 1),
            PluginVersion::new(2, 0, 0),
        ];
        for w in versions.windows(2) {
            assert!(w[0] < w[1]);
        }
    }

    #[test]
    fn version_compatible_same_major_true() {
        let v1 = PluginVersion::new(1, 0, 0);
        let v2 = PluginVersion::new(1, 5, 2);
        assert!(v1.is_compatible_with(&v2));
    }

    #[test]
    fn version_compatible_different_major_false() {
        let v1 = PluginVersion::new(1, 0, 0);
        let v2 = PluginVersion::new(2, 0, 0);
        assert!(!v1.is_compatible_with(&v2));
    }

    #[test]
    fn version_compatible_reflexive() {
        let v = PluginVersion::new(3, 7, 1);
        assert!(v.is_compatible_with(&v));
    }

    #[test]
    fn version_compatible_symmetric() {
        let v1 = PluginVersion::new(1, 0, 0);
        let v2 = PluginVersion::new(1, 9, 0);
        assert_eq!(v1.is_compatible_with(&v2), v2.is_compatible_with(&v1));
    }

    #[test]
    fn version_display_format() {
        let v = PluginVersion::new(2, 1, 3);
        assert_eq!(format!("{v}"), "2.1.3");
    }

    #[test]
    fn version_max_values() {
        let v = PluginVersion::new(u32::MAX, u32::MAX, u32::MAX);
        assert_eq!(v.major(), u32::MAX);
        assert_eq!(v.minor(), u32::MAX);
        assert_eq!(v.patch(), u32::MAX);
    }
}
