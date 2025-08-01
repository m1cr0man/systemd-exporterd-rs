use std::time::Duration;

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
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct StateLabels {
    pub name: String,
    pub machine: String,
    pub state: String,
}

fn get_labels(unit: &crate::service::Unit) -> UnitLabels {
    UnitLabels {
        name: unit.name.clone(),
        machine: unit.machine.clone(),
    }
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

    pub fn record_unit(&mut self, unit: &crate::service::Unit) {
        self.active_state
            .get_or_create(&StateLabels {
                name: unit.name.clone(),
                machine: unit.machine.clone(),
                state: unit.active_state.clone(),
            })
            .inc();

        self.sub_state
            .get_or_create(&StateLabels {
                name: unit.name.clone(),
                machine: unit.machine.clone(),
                state: unit.sub_state.clone(),
            })
            .inc();

        let labels = &get_labels(unit);
        record_gauge(&mut self.start_ts, labels, unit.start_ts);
        record_gauge(&mut self.stop_ts, labels, unit.stop_ts);
        record_gauge(&mut self.main_pid, labels, unit.main_pid.into());

        self.record_service(unit);
    }

    fn record_service(&mut self, unit: &crate::service::Unit) {
        if !unit.is_service() {
            return;
        }
        let labels = &get_labels(unit);
        if let Some(io_stats) = unit.io_stats.as_ref() {
            record_counter(
                &mut self.io_read_bytes_total,
                labels,
                io_stats.read_bytes,
                unit.restarted,
            );
            record_counter(
                &mut self.io_write_bytes_total,
                labels,
                io_stats.write_bytes,
                unit.restarted,
            );
            record_counter(
                &mut self.io_read_ops_total,
                labels,
                io_stats.read_ops,
                unit.restarted,
            );
            record_counter(
                &mut self.io_write_ops_total,
                labels,
                io_stats.write_ops,
                unit.restarted,
            );
        }
        if let Some(ip_stats) = unit.ip_stats.as_ref() {
            record_counter(
                &mut self.ip_egress_bytes_total,
                labels,
                ip_stats.egress_bytes,
                unit.restarted,
            );
            record_counter(
                &mut self.ip_ingress_bytes_total,
                labels,
                ip_stats.ingress_bytes,
                unit.restarted,
            );
            record_counter(
                &mut self.ip_egress_packets_total,
                labels,
                ip_stats.egress_packets,
                unit.restarted,
            );
            record_counter(
                &mut self.ip_ingress_packets_total,
                labels,
                ip_stats.ingress_packets,
                unit.restarted,
            );
        }
        if let Some(cpu_stats) = unit.cpu_stats.as_ref() {
            record_counter(
                &mut self.cpu_usage_nsec_total,
                labels,
                cpu_stats.usage_nsec,
                unit.restarted,
            );
        }
        if let Some(mem_stats) = unit.mem_stats.as_ref() {
            record_gauge(&mut self.mem_current, labels, mem_stats.current);
            record_gauge(&mut self.mem_available, labels, mem_stats.available);
            record_gauge(&mut self.mem_peak, labels, mem_stats.peak);
            record_gauge(&mut self.mem_swap, labels, mem_stats.swap);
            record_gauge(&mut self.mem_swap_peak, labels, mem_stats.swap_peak);
        }
        if let Some(tasks_stats) = unit.tasks_stats.as_ref() {
            record_gauge(&mut self.task_count, labels, tasks_stats.count);
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
