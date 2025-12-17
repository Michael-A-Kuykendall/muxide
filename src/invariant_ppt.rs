//! Invariant PPT Testing Framework
//!
//! This module provides runtime invariant checking and contract test support
//! for Predictive Property-Based Testing (PPT).
//!
//! # Usage
//!
//! ```rust,ignore
//! use muxide::invariant_ppt::*;
//!
//! // In production code - assert invariants
//! assert_invariant!(
//!     box_size == payload.len() + 8,
//!     "Box size must equal header + payload"
//! );
//!
//! // In tests - verify contracts are enforced
//! #[test]
//! fn contract_mp4_boxes() {
//!     contract_test("mp4 boxes", &[
//!         "Box size must equal header + payload",
//!     ]);
//! }
//! ```

use std::collections::HashSet;
use std::sync::{OnceLock, RwLock};

fn invariant_log() -> &'static RwLock<HashSet<String>> {
    static INVARIANT_LOG: OnceLock<RwLock<HashSet<String>>> = OnceLock::new();
    INVARIANT_LOG.get_or_init(|| RwLock::new(HashSet::new()))
}

/// Assert an invariant and log it for contract testing.
///
/// # Arguments
/// * `condition` - The invariant condition (must be true)
/// * `message` - Description of the invariant
/// * `context` - Optional context (module/function name)
///
/// # Panics
/// Panics if the condition is false.
#[macro_export]
macro_rules! assert_invariant {
    ($condition:expr, $message:expr) => {
        $crate::invariant_ppt::__assert_invariant_impl($condition, $message, None)
    };
    ($condition:expr, $message:expr, $context:expr) => {
        $crate::invariant_ppt::__assert_invariant_impl($condition, $message, Some($context))
    };
}

/// Internal implementation - do not call directly
#[doc(hidden)]
pub fn __assert_invariant_impl(condition: bool, message: &str, context: Option<&str>) {
    // Log that this invariant was checked (ignore poisoned lock)
    if let Ok(mut log) = invariant_log().write() {
        log.insert(message.to_string());
    }

    if !condition {
        let ctx = context.unwrap_or("unknown");
        panic!("INVARIANT VIOLATION [{}]: {}", ctx, message);
    }
}

/// Check that specific invariants were verified during test execution.
///
/// # Arguments
/// * `test_name` - Name of the contract test
/// * `required_invariants` - List of invariant messages that must have been checked
///
/// # Panics
/// Panics if any required invariant was not checked.
pub fn contract_test(test_name: &str, required_invariants: &[&str]) {
    let log = match invariant_log().read() {
        Ok(l) => l,
        Err(poisoned) => poisoned.into_inner(),
    };

    let mut missing: Vec<&str> = Vec::new();
    for invariant in required_invariants {
        if !log.contains(*invariant) {
            missing.push(invariant);
        }
    }

    if !missing.is_empty() {
        panic!(
            "CONTRACT FAILURE [{}]: The following invariants were not checked:\n  - {}",
            test_name,
            missing.join("\n  - ")
        );
    }
}

/// Clear the invariant log (call between test runs if needed)
pub fn clear_invariant_log() {
    if let Ok(mut log) = invariant_log().write() {
        log.clear();
    }
}

/// Get a snapshot of currently logged invariants (for debugging)
pub fn get_logged_invariants() -> Vec<String> {
    match invariant_log().read() {
        Ok(log) => log.iter().cloned().collect(),
        Err(poisoned) => poisoned.into_inner().iter().cloned().collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_poisoned_lock_paths_are_handled() {
        clear_invariant_log();

        let _ = std::panic::catch_unwind(|| {
            let mut log = invariant_log().write().unwrap();
            log.insert("poisoned invariant".to_string());
            panic!("poison the lock");
        });

        // These calls should use the poisoned.into_inner() paths.
        contract_test("poisoned", &["poisoned invariant"]);

        let logged = get_logged_invariants();
        assert!(logged.contains(&"poisoned invariant".to_string()));
    }

    #[test]
    fn test_invariant_passes() {
        clear_invariant_log();
        assert_invariant!(true, "test invariant passes");

        let logged = get_logged_invariants();
        assert!(logged.contains(&"test invariant passes".to_string()));
    }

    #[test]
    #[should_panic(expected = "INVARIANT VIOLATION")]
    fn test_invariant_fails() {
        assert_invariant!(false, "this should fail", "test");
    }

    #[test]
    fn test_contract_passes() {
        clear_invariant_log();
        assert_invariant!(true, "contract required invariant");
        contract_test("test contract", &["contract required invariant"]);
    }

    #[test]
    #[should_panic(expected = "CONTRACT FAILURE")]
    fn test_contract_fails_missing() {
        clear_invariant_log();
        // Don't check any invariants
        contract_test("test missing", &["this invariant was never checked"]);
    }
}
