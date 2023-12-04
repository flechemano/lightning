//! Types used by the reputation interface.

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Contains the peer measurements that node A has about node B, that
/// will be taken into account when computing B's reputation score.
#[derive(
    Clone, Debug, Eq, PartialEq, Default, Hash, Serialize, Deserialize, schemars::JsonSchema,
)]
pub struct ReputationMeasurements {
    pub latency: Option<Duration>,
    pub interactions: Option<i64>,
    pub inbound_bandwidth: Option<u128>,
    pub outbound_bandwidth: Option<u128>,
    pub bytes_received: Option<u128>,
    pub bytes_sent: Option<u128>,
    pub uptime: Option<u8>,
    pub hops: Option<u8>,
}

impl ReputationMeasurements {
    pub fn verify(&self) -> bool {
        if let Some(latency) = &self.latency {
            if *latency >= Duration::from_secs(20) {
                return false;
            }
        }
        // TODO: validate interactions score
        if let Some(inbound_bandwidth) = &self.inbound_bandwidth {
            // 6250000000 bytes/s are 50 Gbps
            if *inbound_bandwidth > 6250000000 {
                return false;
            }
        }
        if let Some(outbound_bandwidth) = &self.outbound_bandwidth {
            // 6250000000 bytes/s are 50 Gbps
            if *outbound_bandwidth > 6250000000 {
                return false;
            }
        }
        // TODO: should bytes received and bytes sent even be included here since we alreay have
        // bandwidth?
        if let Some(uptime) = &self.uptime {
            if *uptime > 100 {
                return false;
            }
        }
        true
    }
}
