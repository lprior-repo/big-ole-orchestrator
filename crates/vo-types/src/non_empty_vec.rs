use serde::{Deserialize, Serialize};

/// A `Vec<T>` guaranteed to contain at least one element.
/// Construction always validates non-emptiness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NonEmptyVec<T>(Vec<T>);

impl<'de, T: Deserialize<'de>> Deserialize<'de> for NonEmptyVec<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let items: Vec<T> = Vec::deserialize(deserializer)?;
        if items.is_empty() {
            Err(serde::de::Error::custom("NonEmptyVec must not be empty"))
        } else {
            Ok(NonEmptyVec(items))
        }
    }
}

impl<T> NonEmptyVec<T> {
    /// Construct from a Vec. Returns Err if the vec is empty.
    pub fn new(items: Vec<T>) -> Result<Self, &'static str> {
        if items.is_empty() {
            Err("NonEmptyVec must not be empty")
        } else {
            Ok(NonEmptyVec(items))
        }
    }

    /// Construct from a Vec without validation.
    /// Panics if the vec is empty.
    pub fn new_unchecked(items: Vec<T>) -> Self {
        assert!(!items.is_empty(), "NonEmptyVec must not be empty");
        NonEmptyVec(items)
    }

    /// Borrow the first element.
    /// SAFETY: NonEmptyVec invariant guarantees self.0 is non-empty.
    pub fn first(&self) -> &T {
        // SAFETY: The NonEmptyVec invariant (established by `new` / `new_unchecked`
        // and preserved by all public methods) guarantees the inner vec is non-empty.
        unsafe { self.0.get_unchecked(0) }
    }

    /// Borrow all elements except the first.
    pub fn rest(&self) -> &[T] {
        &self.0[1..]
    }

    /// Borrow the full inner slice.
    pub fn as_slice(&self) -> &[T] {
        &self.0
    }

    /// Consume and return the inner Vec.
    pub fn into_vec(self) -> Vec<T> {
        self.0
    }

    /// Number of elements.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Always false (by invariant).
    pub fn is_empty(&self) -> bool {
        false
    }
}

impl<T> IntoIterator for NonEmptyVec<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // B-1: NonEmptyVec accepts non-empty vec when constructed via new()
    #[test]
    fn non_empty_vec_accepts_non_empty_when_constructed() -> Result<(), &'static str> {
        let items = vec![1, 2, 3];
        let nev = NonEmptyVec::new(items)?;
        assert_eq!(nev.first(), &1);
        assert_eq!(nev.len(), 3);
        assert_eq!(nev.as_slice(), &[1, 2, 3]);
        Ok(())
    }

    // B-2: NonEmptyVec rejects empty vec with error when constructed via new()
    #[test]
    fn non_empty_vec_rejects_empty_when_constructed() {
        let items: Vec<i32> = vec![];
        let result = NonEmptyVec::new(items);
        assert_eq!(result, Err("NonEmptyVec must not be empty"));
    }

    // B-3: NonEmptyVec borrows first element when first() called
    #[test]
    fn non_empty_vec_returns_first_element_when_first_called() {
        let nev = NonEmptyVec::new_unchecked(vec![42]);
        assert_eq!(nev.first(), &42);
    }

    // B-4: NonEmptyVec borrows all but first when rest() called
    #[test]
    fn non_empty_vec_returns_rest_excluding_first_when_rest_called() {
        let nev = NonEmptyVec::new_unchecked(vec![1, 2, 3]);
        assert_eq!(nev.rest(), &[2, 3]);
    }

    // B-5: NonEmptyVec borrows full inner slice when as_slice() called
    #[test]
    fn non_empty_vec_returns_full_slice_when_as_slice_called() {
        let nev = NonEmptyVec::new_unchecked(vec![10, 20, 30]);
        assert_eq!(nev.as_slice(), &[10, 20, 30]);
    }

    // B-6: NonEmptyVec consumes into inner vec when into_vec() called
    #[test]
    fn non_empty_vec_returns_inner_vec_when_into_vec_called() {
        let nev = NonEmptyVec::new_unchecked(vec![1, 2]);
        assert_eq!(nev.into_vec(), vec![1, 2]);
    }

    // B-7: NonEmptyVec returns correct count when len() called
    #[test]
    fn non_empty_vec_returns_element_count_when_len_called() {
        let nev = NonEmptyVec::new_unchecked(vec![1, 2, 3, 4, 5]);
        assert_eq!(nev.len(), 5);
    }

    // B-8: NonEmptyVec is_empty() always returns false
    #[test]
    fn non_empty_vec_is_empty_always_returns_false_when_called() {
        let nev = NonEmptyVec::new_unchecked(vec![42]);
        assert!(!nev.is_empty());
    }

    // B-9: NonEmptyVec constructs without validation when new_unchecked() with non-empty
    #[test]
    fn non_empty_vec_new_unchecked_constructs_when_vec_is_non_empty() {
        let nev = NonEmptyVec::new_unchecked(vec![99]);
        assert_eq!(nev.first(), &99);
        assert_eq!(nev.len(), 1);
        assert_eq!(nev.as_slice(), &[99]);
    }

    // B-10: NonEmptyVec new_unchecked() panics when called with empty vec
    #[test]
    #[should_panic(expected = "NonEmptyVec")]
    fn non_empty_vec_new_unchecked_panics_when_vec_is_empty() {
        NonEmptyVec::new_unchecked(Vec::<i32>::new());
    }

    // B-63: NonEmptyVec yields all elements in insertion order when consumed via IntoIterator
    #[test]
    fn non_empty_vec_yields_all_elements_in_order_when_iterated() {
        let nev = NonEmptyVec::new_unchecked(vec![10, 20, 30]);
        let collected: Vec<i32> = nev.into_iter().collect();
        assert_eq!(collected, vec![10, 20, 30]);
    }

    // B-64: NonEmptyVec yields exactly one element when singleton consumed via IntoIterator
    #[test]
    fn non_empty_vec_yields_single_element_when_singleton_iterated() {
        let nev = NonEmptyVec::new_unchecked(vec![42]);
        let collected: Vec<i32> = nev.into_iter().collect();
        assert_eq!(collected, vec![42]);
    }

    // Additional: rest() returns empty slice for singleton
    #[test]
    fn non_empty_vec_rest_returns_empty_slice_when_singleton() {
        let nev = NonEmptyVec::new_unchecked(vec![99]);
        assert_eq!(nev.rest(), &[] as &[i32]);
    }

    // NonEmptyVec deserialization rejects empty JSON array
    #[test]
    fn non_empty_vec_deserialization_rejects_empty_json_array_when_input_is_empty() {
        let json = serde_json::json!([]);
        let result = serde_json::from_value::<NonEmptyVec<String>>(json);
        let err = result.expect_err("should fail for empty JSON array");
        assert!(err.to_string().contains("NonEmptyVec must not be empty"));
    }

    // Proptest: NonEmptyVec serde round-trip
    mod proptests {
        use super::*;

        proptest! {
            /// Invariant: For any NonEmptyVec<Vec<u8>> with 1..=100 elements,
            /// serialize then deserialize preserves the original.
            #[test]
            fn non_empty_vec_serde_round_trip_proptest(
                v in proptest::collection::vec(any::<u8>(), 1..=100)
            ) {
                let nev = NonEmptyVec::new_unchecked(v.clone());
                let json = serde_json::to_value(&nev).expect("serialize");
                let restored: NonEmptyVec<u8> = serde_json::from_value(json).expect("deserialize");
                prop_assert_eq!(restored.as_slice(), v.as_slice());
            }
        }
    }
}
