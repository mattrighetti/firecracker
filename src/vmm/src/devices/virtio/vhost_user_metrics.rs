// Copyright 2023 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

//! Defines the metrics system for vhost-user devices.
//!
//! # Metrics format
//! The metrics are flushed in JSON when requested by vmm::logger::metrics::METRICS.write().
//!
//! ## JSON example with metrics:
//! ```json
//! {
//!  "vhost_user_{mod}_id0": {
//!     "activate_fails": "SharedIncMetric",
//!     "cfg_fails": "SharedIncMetric",
//!     "init_time_us": SharedStoreMetric,
//!     "activate_time_us": SharedStoreMetric,
//!  }
//!  "vhost_user_{mod}_id1": {
//!     "activate_fails": "SharedIncMetric",
//!     "cfg_fails": "SharedIncMetric",
//!     "init_time_us": SharedStoreMetric,
//!     "activate_time_us": SharedStoreMetric,
//!  }
//!  ...
//!  "vhost_user_{mod}_idN": {
//!     "activate_fails": "SharedIncMetric",
//!     "cfg_fails": "SharedIncMetric",
//!     "init_time_us": SharedStoreMetric,
//!     "activate_time_us": SharedStoreMetric,
//!  }
//! }
//! ```
//! Each `vhost_user` field in the example above is a serializable `VhostUserDeviceMetrics`
//! structure collecting metrics such as `activate_fails`, `cfg_fails`, `init_time_us` and
//! `activate_time_us` for the vhost_user device.
//! For vhost-user block device having endpoint "/drives/drv0" the emitted metrics would be
//! `vhost_user_block_drv0`.
//! For vhost-user block device having endpoint "/drives/drvN" the emitted metrics would be
//! `vhost_user_block_drvN`.
//! Aggregate metrics for `vhost_user` if `not` emitted as it can be easily obtained in
//! typical observability tools.
//!
//! # Design
//! The main design goals of this system are:
//! * To improve vhost_user device metrics by logging them at per device granularity.
//! * `vhost_user` is a new device with no metrics emitted before so, backward compatibility doesn't
//!   come into picture like it was in the case of block/net devices. And since, metrics can be
//!   easily aggregated using typical observability tools, we chose not to provide aggregate
//!   vhost_user metrics.
//! * Rely on `serde` to provide the actual serialization for writing the metrics.
//! * Since all metrics start at 0, we implement the `Default` trait via derive for all of them, to
//!   avoid having to initialize everything by hand.
//!
//! * Follow the design of Block and Net device metrics and use a map of vhost_user device name and
//!   corresponding metrics.
//! * Metrics are flushed with key `vhost_user_{module_specific_name}` and each module sets an
//!   appropriate `module_specific_name` in the format `{mod}_{id}`. e.g. vhost-user block device in
//!   this commit set this as `format!("{}_{}", "block_", config.drive_id.clone());` This way
//!   vhost_user_metrics stay generic while the specific vhost_user devices can have their unique
//!   metrics.
//!
//! The system implements 2 type of metrics:
//! * Shared Incremental Metrics (SharedIncMetrics) - dedicated for the metrics which need a counter
//! (i.e the number of times activating a device failed). These metrics are reset upon flush.
//! * Shared Store Metrics (SharedStoreMetrics) - are targeted at keeping a persistent value, it is
//!   `not` intended to act as a counter (i.e for measure the process start up time for example).
//! We add VhostUserDeviceMetrics entries from vhost_user_metrics::METRICS into vhost_user device
//! instead of vhost_user device having individual separate VhostUserDeviceMetrics entries because
//! vhost_user device is not accessible from signal handlers to flush metrics and
//! vhost_user_metrics::METRICS is.

use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

use serde::ser::SerializeMap;
use serde::{Serialize, Serializer};

use crate::logger::{SharedIncMetric, SharedStoreMetric};

/// map of vhost_user drive id and metrics
/// this should be protected by a lock before accessing.
#[allow(missing_debug_implementations)]
pub struct VhostUserMetricsPerDevice {
    /// used to access per vhost_user device metrics
    pub metrics: BTreeMap<String, Arc<VhostUserDeviceMetrics>>,
}

impl VhostUserMetricsPerDevice {
    /// Allocate `VhostUserDeviceMetrics` for vhost_user device having
    /// id `drive_id`. Also, allocate only if it doesn't
    /// exist to avoid overwriting previously allocated data.
    /// lock is always initialized so it is safe the unwrap
    /// the lock without a check.
    pub fn alloc(drive_id: String) -> Arc<VhostUserDeviceMetrics> {
        if METRICS.read().unwrap().metrics.get(&drive_id).is_none() {
            METRICS.write().unwrap().metrics.insert(
                drive_id.clone(),
                Arc::new(VhostUserDeviceMetrics::default()),
            );
        }
        METRICS
            .read()
            .unwrap()
            .metrics
            .get(&drive_id)
            .unwrap()
            .clone()
    }
}

/// Pool of vhost_user-related metrics per device behind a lock to
/// keep things thread safe. Since the lock is initialized here
/// it is safe to unwrap it without any check.
static METRICS: RwLock<VhostUserMetricsPerDevice> = RwLock::new(VhostUserMetricsPerDevice {
    metrics: BTreeMap::new(),
});

/// This function facilitates serialization of vhost_user device metrics.
pub fn flush_metrics<S: Serializer>(serializer: S) -> Result<S::Ok, S::Error> {
    let vhost_user_metrics = METRICS.read().unwrap();
    let metrics_len = vhost_user_metrics.metrics.len();
    let mut seq = serializer.serialize_map(Some(metrics_len))?;

    for (name, metrics) in vhost_user_metrics.metrics.iter() {
        let devn = format!("vhost_user_{}", name);
        seq.serialize_entry(&devn, metrics)?;
    }
    seq.end()
}

/// vhost_user Device associated metrics.
#[derive(Debug, Default, Serialize)]
pub struct VhostUserDeviceMetrics {
    /// Number of times when activate failed on a vhost_user device.
    pub activate_fails: SharedIncMetric,
    /// Number of times when interacting with the space config of a vhost-user device failed.
    pub cfg_fails: SharedIncMetric,
    // Vhost-user init time in microseconds.
    pub init_time_us: SharedStoreMetric,
    // Vhost-user activate time in microseconds.
    pub activate_time_us: SharedStoreMetric,
}