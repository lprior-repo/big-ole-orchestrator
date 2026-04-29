use std::fmt;

use serde::{Deserialize, Serialize};

use crate::types::FenceToken;

use super::types::{
    CapabilityId, PluginId, PluginName, PluginVersion, PluginVersionConstraint, ResourceBudget,
    SchemaVersion,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PluginErrorCategory {
    RegistrationFailure,
    LoadFailure,
    ActivationFailure,
    DependencyFailure,
    VersionIncompatibility,
    ResourceExhaustion,
    QuiesceTimeout,
    FenceViolation,
    IsolationViolation,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PluginErrorDetail {
    PluginNotFound(PluginId),
    PluginAlreadyLoaded(PluginId),
    SchemaVersionMismatch {
        expected: SchemaVersion,
        actual: PluginVersion,
    },
    CapabilityNotSatisfied {
        plugin_id: PluginId,
        missing: CapabilityId,
    },
    DependencyCycle(Vec<PluginName>),
    UnsatisfiedDependency {
        plugin_id: PluginId,
        missing: PluginVersionConstraint,
    },
    ResourceBudgetExceeded {
        plugin_id: PluginId,
        required: ResourceBudget,
        available: ResourceBudget,
    },
    QuiesceDeadlineExceeded(PluginId),
    FenceRegression {
        plugin_id: PluginId,
        presented_token: FenceToken,
        current_token: FenceToken,
    },
    IsolationBreach {
        plugin_id: PluginId,
        violation_type: IsolationBreachType,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IsolationBreachType {
    CrossBoundaryAccess,
    SharedMemoryViolation,
    UnauthorizedCapabilityUse,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[allow(clippy::enum_variant_names)]
pub enum PluginErrorContext {
    DuringRegistration,
    DuringLoad,
    DuringActivation,
    DuringQuiesce,
    DuringUnload,
    DuringHealthCheck,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, thiserror::Error)]
pub struct PluginHotLoadError {
    pub category: PluginErrorCategory,
    pub detail: PluginErrorDetail,
    pub context: PluginErrorContext,
}

impl PluginHotLoadError {
    #[must_use]
    pub fn new(
        category: PluginErrorCategory,
        detail: PluginErrorDetail,
        context: PluginErrorContext,
    ) -> Self {
        Self {
            category,
            detail,
            context,
        }
    }

    #[must_use]
    pub fn category(&self) -> &PluginErrorCategory {
        &self.category
    }

    #[must_use]
    pub fn detail(&self) -> &PluginErrorDetail {
        &self.detail
    }

    #[must_use]
    pub fn context(&self) -> &PluginErrorContext {
        &self.context
    }
}

impl fmt::Display for PluginHotLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let category = format!("{:?}", self.category);
        let context = format!("{:?}", self.context);
        let mut category_msg = String::new();
        for c in category.chars() {
            if c.is_uppercase() && !category_msg.is_empty() {
                let prev = category_msg.chars().last().map_or('_', |ch| ch);
                if prev != '_' && prev != ' ' {
                    category_msg.push(' ');
                }
            }
            for lc in c.to_lowercase() {
                category_msg.push(lc);
            }
        }
        let context_msg = context
            .strip_prefix("During")
            .unwrap_or(&context)
            .to_lowercase();
        write!(f, "{category_msg} during {context_msg}")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IsolationLevel {
    SharedRuntime,
    IsolatedActor,
    Process,
}
