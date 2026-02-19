#[derive(Default)]
pub struct IOStats {
    pub read_bytes: u64,
    pub write_bytes: u64,
    pub read_ops: u64,
    pub write_ops: u64,
}

#[derive(Default)]
pub struct IPStats {
    pub egress_bytes: u64,
    pub ingress_bytes: u64,
    pub egress_packets: u64,
    pub ingress_packets: u64,
}

#[derive(Default)]
pub struct CPUStats {
    pub usage_nsec: u64,
}

#[derive(Default)]
pub struct MemoryStats {
    pub current: u64,
    pub available: u64,
    pub peak: u64,
    pub swap: u64,
    pub swap_peak: u64,
}

#[derive(Default)]
pub struct TaskStats {
    pub count: u64,
    pub main_pid: u32,
    pub start_ts: u64,
    pub stop_ts: u64,
}

#[derive(Default)]
pub struct UnitStatus {
    pub active_state: String,
    pub sub_state: String,
    pub active_ts: u64,
    pub inactive_ts: u64,
}
