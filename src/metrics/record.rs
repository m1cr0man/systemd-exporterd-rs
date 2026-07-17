use std::fmt::Debug;

use prometheus_client::metrics::{counter::Counter, family::Family, gauge::Gauge};

pub(super) fn record_gauge<S: Clone + std::hash::Hash + Eq + Debug>(
    gauge: &mut Family<S, Gauge>,
    labels: &S,
    value: u64,
) {
    let value: i64 = match value.try_into() {
        Ok(v) => v,
        Err(err) => {
            if value == u64::MAX {
                i64::MAX
            } else {
                println!(
                    "Failed to convert to i64: labels: {:?} value: {:?} error: {:?}",
                    labels, value, err
                );
                return;
            }
        }
    };
    gauge.get_or_create(labels).set(value);
}

pub(super) fn record_counter<S: Clone + std::hash::Hash + Eq + Debug>(
    counter: &mut Family<S, Counter>,
    labels: &S,
    value: u64,
) {
    counter.get_or_create(labels).inc_by(value);
}
