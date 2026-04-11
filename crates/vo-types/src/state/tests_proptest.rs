//! Property-based tests for LeaseRecord immutability.

use super::LeaseRecord;

proptest::proptest! {
    #[test]
    fn leaserecord_immutability_proptest(i in ".*", s in ".*", t in 1u64..) {
        let instance = crate::string_types::InstanceId(i);
        let step = crate::string_types::StepId(s);
        let token = crate::integer_types::FenceToken(std::num::NonZeroU64::new(t).unwrap());

        let rec = LeaseRecord::new(instance.clone(), step.clone(), token);
        proptest::prop_assert_eq!(rec.instance_id(), &instance);
        proptest::prop_assert_eq!(rec.step_id(), &step);
        proptest::prop_assert_eq!(rec.token(), &token);
    }
}
