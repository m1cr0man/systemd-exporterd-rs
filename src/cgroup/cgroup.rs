use std::path::PathBuf;

use snafu::ResultExt;

use crate::stats::{CPUStats, IOStats, MemoryStats, TaskStats};

pub struct CGroup {
    path: PathBuf,
}

impl CGroup {
    pub fn new(path: &str) -> Self {
        Self {
            path: PathBuf::from(format!("{}{}", super::constants::CGROUP_ROOT, path)),
        }
    }

    fn read_single_stat(&self, name: &str) -> Result<u64, super::Error> {
        Ok(std::fs::read_to_string(self.path.join(name))
            .context(super::IOSnafu)?
            .parse()
            .unwrap_or_default())
    }

    pub fn read_cpu_stats(&self) -> Result<CPUStats, super::Error> {
        let mut stats = CPUStats::default();
        let raw_stats =
            std::fs::read_to_string(self.path.join("cpu.stat")).context(super::IOSnafu)?;

        // Optimising the iteration of "key value" pairs.
        // Using zip is more optimal than chunks as the tuple size is known at compile time (2)
        let data = raw_stats.split_ascii_whitespace();
        let mut data_clone = data.clone();
        data_clone.next();
        for (key, val) in data.zip(data_clone) {
            match key {
                "usage_nsec" => {
                    stats.usage_nsec = val.parse().unwrap_or_default();
                    break;
                }
                _ => {}
            }
        }

        Ok(stats)
    }

    pub fn read_memory_stats(&self) -> Result<MemoryStats, super::Error> {
        let mut stats = MemoryStats::default();
        stats.current = self.read_single_stat("memory.current")?;
        stats.peak = self.read_single_stat("memory.peak")?;
        stats.swap = self.read_single_stat("memory.swap.current")?;
        stats.swap_peak = self.read_single_stat("memory.swap.peak")?;
        Ok(stats)
    }

    pub fn read_io_stats(&self) -> Result<IOStats, super::Error> {
        let mut stats = IOStats::default();
        let raw_stats =
            std::fs::read_to_string(self.path.join("io.stat")).context(super::IOSnafu)?;

        for line in raw_stats.split("\n") {
            let mut kvs = line.split_ascii_whitespace();
            // Skip the device ID (N:M)
            kvs.next();
            for kv in kvs {
                let (key, val) = kv.split_once("=").unwrap();
                let val_parsed = || val.parse::<u64>().unwrap_or_default();
                match key {
                    "rbytes" => stats.read_bytes += val_parsed(),
                    "wbytes" => stats.write_bytes += val_parsed(),
                    "rios" => stats.read_ops += val_parsed(),
                    "wios" => stats.write_ops += val_parsed(),
                    _ => {}
                }
            }
        }

        Ok(stats)
    }

    pub fn read_task_stats(&self) -> Result<TaskStats, super::Error> {
        let mut stats = TaskStats::default();
        stats.count = self.read_single_stat("pids.current")?;
        Ok(stats)
    }
}
