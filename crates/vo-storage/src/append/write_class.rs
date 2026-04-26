use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WriteClass {
    CriticalControlPlane,
    OperatorProjection,
    BulkBlob,
}

impl WriteClass {
    #[must_use]
    pub const fn tier(self) -> u8 {
        match self {
            Self::CriticalControlPlane => 1,
            Self::OperatorProjection => 2,
            Self::BulkBlob => 3,
        }
    }

    #[must_use]
    pub const fn never_drops(self) -> bool {
        matches!(self, Self::CriticalControlPlane)
    }
}

impl std::str::FromStr for WriteClass {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "critical_control_plane" => Ok(Self::CriticalControlPlane),
            "operator_projection" => Ok(Self::OperatorProjection),
            "bulk_blob" => Ok(Self::BulkBlob),
            _ => Err(format!("unknown write class: {s}")),
        }
    }
}
