use crate::codec::StorageError;
use vo_types::{InstanceId, TimerId};

type ScanResult = Vec<(Vec<u8>, Vec<u8>)>;

pub struct TimerKey([u8; 40]);

impl TimerKey {
    /// Creates a new `TimerKey` from fire time, instance ID, and timer ID.
    ///
    /// # Errors
    ///
    /// Returns `StorageError::InvalidArgument` if `instance_id` or `timer_id` cannot be converted to bytes.
    #[allow(clippy::needless_pass_by_value)]
    pub fn new(
        fire_at_ms: u64,
        instance_id: InstanceId,
        timer_id: TimerId,
    ) -> Result<Self, StorageError> {
        let mut bytes = [0u8; 40];
        bytes[0..8].copy_from_slice(&fire_at_ms.to_be_bytes());
        bytes[8..24].copy_from_slice(
            &instance_id
                .to_bytes()
                .map_err(|_| StorageError::InvalidArgument)?,
        );
        bytes[24..40].copy_from_slice(
            &timer_id
                .to_bytes()
                .map_err(|_| StorageError::InvalidArgument)?,
        );
        Ok(Self(bytes))
    }
    #[must_use]
    pub const fn fire_at_ms(&self) -> u64 {
        u64::from_be_bytes([
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5], self.0[6], self.0[7],
        ])
    }
    #[must_use]
    pub fn instance_id(&self) -> InstanceId {
        let bytes: [u8; 16] = [
            self.0[8], self.0[9], self.0[10], self.0[11], self.0[12], self.0[13], self.0[14],
            self.0[15], self.0[16], self.0[17], self.0[18], self.0[19], self.0[20], self.0[21],
            self.0[22], self.0[23],
        ];
        InstanceId::from_bytes(bytes)
    }
    #[must_use]
    pub fn timer_id(&self) -> TimerId {
        let bytes: [u8; 16] = [
            self.0[24], self.0[25], self.0[26], self.0[27], self.0[28], self.0[29], self.0[30],
            self.0[31], self.0[32], self.0[33], self.0[34], self.0[35], self.0[36], self.0[37],
            self.0[38], self.0[39],
        ];
        TimerId::from_bytes(bytes)
    }
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 40] {
        &self.0
    }
}
