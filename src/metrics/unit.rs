use std::time::Duration;

use crate::stats::{ResourceStats, UnitData};
use prometheus_client::encoding::EncodeLabelSet;
use prometheus_client::metrics::counter::Counter;
use prometheus_client::metrics::family::Family;
use prometheus_client::metrics::gauge::Gauge;
use prometheus_client::registry::Registry;

use super::record::{record_counter, record_gauge};

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct UnitLabels {
    pub name: String,
    pub machine: String,
    pub scope: String,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct StateLabels {
    pub name: String,
    pub machine: String,
    pub scope: String,
    pub state: String,
}

#[derive(Default, Clone)]
pub struct UnitMetrics {
    pub scrape_time_us: Counter,
    pub scrape_count: Counter,
    pub active_state: Family<StateLabels, Counter>,
    pub sub_state: Family<StateLabels, Counter>,
    pub active_ts: Family<UnitLabels, Gauge>,
    pub inactive_ts: Family<UnitLabels, Gauge>,
    pub start_ts: Family<UnitLabels, Gauge>,
    pub stop_ts: Family<UnitLabels, Gauge>,
    pub main_pid: Family<UnitLabels, Gauge>,
    pub job_id: Family<UnitLabels, Gauge>,

    // IOStats
    pub io_read_bytes_total: Family<UnitLabels, Counter>,
    pub io_write_bytes_total: Family<UnitLabels, Counter>,
    pub io_read_ops_total: Family<UnitLabels, Counter>,
    pub io_write_ops_total: Family<UnitLabels, Counter>,

    // IPStats
    pub ip_egress_bytes_total: Family<UnitLabels, Counter>,
    pub ip_ingress_bytes_total: Family<UnitLabels, Counter>,
    pub ip_egress_packets_total: Family<UnitLabels, Counter>,
    pub ip_ingress_packets_total: Family<UnitLabels, Counter>,

    // CPUStats
    pub cpu_usage_nsec_total: Family<UnitLabels, Counter>,

    // MemoryStats
    pub mem_current: Family<UnitLabels, Gauge>,
    pub mem_available: Family<UnitLabels, Gauge>,
    pub mem_peak: Family<UnitLabels, Gauge>,
    pub mem_swap: Family<UnitLabels, Gauge>,
    pub mem_swap_peak: Family<UnitLabels, Gauge>,

    // TaskStats
    pub task_count: Family<UnitLabels, Gauge>,
}

impl UnitMetrics {
    pub fn register_metrics(self, registry: &mut Registry) {
        registry.register(
            "scrape_time_us",
            "The time taken in microseconds to perform the scrape.",
            self.scrape_time_us,
        );
        registry.register(
            "scrape_count",
            "The number of times scraping has been performed.",
            self.scrape_count,
        );
        registry.register(
            "active_state",
            "The active state of the unit (e.g., 'active', 'inactive', 'failed', 'reloading').",
            self.active_state,
        );
        registry.register(
            "sub_state",
            "A more detailed sub-state of the unit within its active state (e.g., 'running', 'dead', 'exited', 'mounted').",
            self.sub_state,
        );
        registry.register(
            "active_ts",
            "The timestamp (in microseconds since the Unix epoch) when the unit became active.",
            self.active_ts,
        );
        registry.register(
            "inactive_ts",
            "The timestamp (in microseconds since the Unix epoch) when the unit became inactive.",
            self.inactive_ts,
        );
        registry.register(
            "start_ts",
            "The timestamp (in microseconds since the Unix epoch) when its main process started.",
            self.start_ts,
        );
        registry.register(
            "stop_ts",
            "The timestamp (in microseconds since the Unix epoch) when its main process exited.",
            self.stop_ts,
        );
        registry.register(
            "main_pid",
            "The Process ID (PID) of the main process associated with the unit, if applicable.",
            self.main_pid,
        );
        registry.register(
            "io_read_bytes_total",
            "The total number of bytes read by the unit.",
            self.io_read_bytes_total,
        );
        registry.register(
            "io_write_bytes_total",
            "The total number of bytes written by the unit.",
            self.io_write_bytes_total,
        );
        registry.register(
            "io_read_ops_total",
            "The total number of read operations performed by the unit.",
            self.io_read_ops_total,
        );
        registry.register(
            "io_write_ops_total",
            "The total number of write operations performed by the unit.",
            self.io_write_ops_total,
        );
        registry.register(
            "ip_egress_bytes_total",
            "The total number of bytes sent (egress) by the unit over the network.",
            self.ip_egress_bytes_total,
        );
        registry.register(
            "ip_ingress_bytes_total",
            "The total number of bytes received (ingress) by the unit over the network.",
            self.ip_ingress_bytes_total,
        );
        registry.register(
            "ip_egress_packets_total",
            "The total number of packets sent (egress) by the unit over the network.",
            self.ip_egress_packets_total,
        );
        registry.register(
            "ip_ingress_packets_total",
            "The total number of packets received (ingress) by the unit over the network.",
            self.ip_ingress_packets_total,
        );
        registry.register(
            "cpu_usage_nsec_total",
            "The total CPU time used by the unit, measured in nanoseconds.",
            self.cpu_usage_nsec_total,
        );
        registry.register(
            "mem_current",
            "The current memory usage of the unit in bytes.",
            self.mem_current,
        );
        registry.register(
            "mem_available",
            "The amount of memory available to the unit in bytes. This might refer to remaining memory from a configured limit, or system-wide available memory for the unit.",
            self.mem_available,
        );
        registry.register(
            "mem_peak",
            "The peak (maximum) memory usage recorded for the unit in bytes since its start.",
            self.mem_peak,
        );
        registry.register(
            "mem_swap",
            "The current amount of swap space used by the unit in bytes.",
            self.mem_swap,
        );
        registry.register(
            "mem_swap_peak",
            "The peak (maximum) swap space usage recorded for the unit in bytes since its start.",
            self.mem_swap_peak,
        );
        registry.register(
            "task_count",
            "The current number of tasks (processes or threads) associated with the unit.",
            self.task_count,
        );
    }

    // new_batch should be called before each recording batch
    pub fn new_batch(&mut self) {
        self.active_state.clear();
        self.sub_state.clear();
    }

    pub fn record_unit(&mut self, data: UnitData) {
        let unit_labels = UnitLabels {
            name: data.name.clone(),
            machine: data.machine.clone(),
            scope: data.scope.clone(),
        };
        let last_job_id = self
            .job_id
            .get_or_create(&unit_labels)
            .set(data.status.job_id as i64);
        let restarted = last_job_id != (data.status.job_id as i64);

        let state_labels = StateLabels {
            name: data.name.clone(),
            machine: data.machine.clone(),
            scope: data.scope.clone(),
            state: data.status.active_state.clone(),
        };
        self.active_state.get_or_create(&state_labels).inc();
        self.sub_state.get_or_create(&state_labels).inc();

        let stats = &data.task_stats;
        if stats.main_pid > 0 {
            record_gauge(&mut self.start_ts, &unit_labels, stats.start_ts);
            record_gauge(&mut self.stop_ts, &unit_labels, stats.stop_ts);
            record_gauge(&mut self.main_pid, &unit_labels, stats.main_pid.into());
            record_gauge(&mut self.task_count, &unit_labels, stats.count);
        }

        self.record_resources(&unit_labels, &data.resource_stats, restarted);
    }

    fn record_resources(&mut self, labels: &UnitLabels, stats: &ResourceStats, restarted: bool) {
        // You will always have some amount of read activity on any unit.
        // If you don't, why bother recording stats?
        if stats.io_stats.read_bytes > 0 {
            record_counter(
                &mut self.io_read_bytes_total,
                &labels,
                stats.io_stats.read_bytes,
                restarted,
            );
            record_counter(
                &mut self.io_write_bytes_total,
                labels,
                stats.io_stats.write_bytes,
                restarted,
            );
            record_counter(
                &mut self.io_read_ops_total,
                labels,
                stats.io_stats.read_ops,
                restarted,
            );
            record_counter(
                &mut self.io_write_ops_total,
                labels,
                stats.io_stats.write_ops,
                restarted,
            );
        } else {
            self.io_read_bytes_total.remove(labels);
            self.io_write_bytes_total.remove(labels);
            self.io_read_ops_total.remove(labels);
            self.io_write_ops_total.remove(labels);
        }
        if stats.ip_stats.egress_packets > 0 {
            record_counter(
                &mut self.ip_egress_bytes_total,
                labels,
                stats.ip_stats.egress_bytes,
                restarted,
            );
            record_counter(
                &mut self.ip_ingress_bytes_total,
                labels,
                stats.ip_stats.ingress_bytes,
                restarted,
            );
            record_counter(
                &mut self.ip_egress_packets_total,
                labels,
                stats.ip_stats.egress_packets,
                restarted,
            );
            record_counter(
                &mut self.ip_ingress_packets_total,
                labels,
                stats.ip_stats.ingress_packets,
                restarted,
            );
        } else {
            self.ip_egress_bytes_total.remove(labels);
            self.ip_ingress_bytes_total.remove(labels);
            self.ip_egress_packets_total.remove(labels);
            self.ip_ingress_packets_total.remove(labels);
        }
        if stats.cpu_stats.usage_nsec > 0 {
            record_counter(
                &mut self.cpu_usage_nsec_total,
                labels,
                stats.cpu_stats.usage_nsec,
                restarted,
            );
        } else {
            self.cpu_usage_nsec_total.remove(labels);
        }
        if stats.mem_stats.current > 0 {
            record_gauge(&mut self.mem_current, labels, stats.mem_stats.current);
            record_gauge(&mut self.mem_available, labels, stats.mem_stats.available);
            record_gauge(&mut self.mem_peak, labels, stats.mem_stats.peak);
            record_gauge(&mut self.mem_swap, labels, stats.mem_stats.swap);
            record_gauge(&mut self.mem_swap_peak, labels, stats.mem_stats.swap_peak);
        } else {
            self.mem_current.remove(labels);
            self.mem_available.remove(labels);
            self.mem_peak.remove(labels);
            self.mem_swap.remove(labels);
            self.mem_swap_peak.remove(labels);
        }
    }

    pub fn record_scrape(&mut self, scrape_time: Duration) {
        self.scrape_time_us
            .inc_by(scrape_time.as_micros().try_into().unwrap_or(0));
        self.scrape_count.inc();
    }
}

pub trait MetricSource {
    fn register_metrics(registry: &mut Registry);
}
