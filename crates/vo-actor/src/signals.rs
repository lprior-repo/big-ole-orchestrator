//! Signal matching predicates for wait-key based signal routing.
//!
//! This module provides types and predicates for determining whether an
//! incoming signal matches a workflow's registered wait-key.

use crate::SignalPayload;
use crate::WaitKey;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signal {
    pub key: WaitKey,
    pub payload: SignalPayload,
}

impl Signal {
    #[must_use]
    pub fn new(key: WaitKey, payload: SignalPayload) -> Self {
        Self { key, payload }
    }

    #[must_use]
    pub fn key(&self) -> &WaitKey {
        &self.key
    }

    #[must_use]
    pub fn payload(&self) -> &SignalPayload {
        &self.payload
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchPredicate {
    Exact(WaitKey),
}

impl MatchPredicate {
    #[must_use]
    pub fn exact(key: WaitKey) -> Self {
        Self::Exact(key)
    }

    #[must_use]
    pub fn matches(&self, signal: &Signal) -> bool {
        match self {
            MatchPredicate::Exact(wait_key) => signal.key == *wait_key,
        }
    }

    #[must_use]
    pub fn wait_key(&self) -> &WaitKey {
        match self {
            MatchPredicate::Exact(key) => key,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wait_key_ok(s: &str) -> WaitKey {
        WaitKey::parse(s).expect("valid wait key")
    }

    fn payload_empty() -> SignalPayload {
        SignalPayload::empty()
    }

    fn signal(key: &str) -> Signal {
        Signal::new(wait_key_ok(key), payload_empty())
    }

    fn predicate(key: &str) -> MatchPredicate {
        MatchPredicate::exact(wait_key_ok(key))
    }

    #[test]
    fn signal_perfectly_matches_string_wait_key() {
        let sig = signal("approval");
        let pred = predicate("approval");
        assert!(pred.matches(&sig));
    }

    #[test]
    fn signal_does_not_match_different_wait_key() {
        let sig = signal("approval");
        let pred = predicate("rejection");
        assert!(!pred.matches(&sig));
    }

    #[test]
    fn different_keys_do_not_match() {
        let sig = signal("key-a");
        let pred = predicate("key-b");
        assert!(!pred.matches(&sig));
    }

    #[test]
    fn same_keys_match() {
        let sig = signal("key-a");
        let pred = predicate("key-a");
        assert!(pred.matches(&sig));
    }

    #[test]
    fn signal_with_payload_matches_when_key_matches() {
        let key = wait_key_ok("webhook");
        let payload = SignalPayload::from_bytes(vec![1, 2, 3]).expect("valid payload");
        let sig = Signal::new(key.clone(), payload);
        let pred = MatchPredicate::exact(key);
        assert!(pred.matches(&sig));
    }

    #[test]
    fn signal_with_payload_does_not_match_when_key_differs() {
        let sig = Signal::new(wait_key_ok("webhook"), payload_empty());
        let pred = predicate("timer");
        assert!(!pred.matches(&sig));
    }

    #[test]
    fn predicate_wait_key_returns_correct_key() {
        let pred = predicate("test-key");
        assert_eq!(pred.wait_key().as_str(), "test-key");
    }

    #[test]
    fn match_predicate_exact_is_debuggable() {
        let pred = predicate("debug-key");
        let debug = format!("{:?}", pred);
        assert!(debug.contains("Exact"));
        assert!(debug.contains("debug-key"));
    }

    #[test]
    fn signal_is_debuggable() {
        let sig = signal("test-signal");
        let debug = format!("{:?}", sig);
        assert!(debug.contains("Signal"));
        assert!(debug.contains("test-signal"));
    }
}
