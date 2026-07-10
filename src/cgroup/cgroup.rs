use std::{
    fmt::{Debug, Display},
    path::PathBuf,
};

use snafu::ResultExt;

use crate::stats::{CPUStats, IOStats, MemoryStats, TaskStats};

const CGROUP_ROOT: &str = &"/sys/fs/cgroup";

pub struct CGroup {
    path: PathBuf,
    pub is_root: bool,
}

impl CGroup {
    pub fn new(path: &str) -> Self {
        Self {
            path: PathBuf::from(format!("{}{}", CGROUP_ROOT, path)),
            is_root: path.trim_matches('/').is_empty(),
        }
    }

    async fn read_single_stat(&self, name: &str) -> Result<u64, super::Error> {
        Ok(tokio::fs::read_to_string(self.path.join(name))
            .await
            .context(super::IOSnafu)?
            .trim()
            .parse()
            .unwrap_or_default())
    }

    pub async fn read_cpu_stats(&self) -> Result<CPUStats, super::Error> {
        let mut stats = CPUStats::default();
        let raw_stats = tokio::fs::read_to_string(self.path.join("cpu.stat"))
            .await
            .context(super::IOSnafu)?;

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

    pub async fn read_memory_stats(&self) -> Result<MemoryStats, super::Error> {
        let mut stats = MemoryStats::default();
        stats.current = self.read_single_stat("memory.current").await?;
        stats.peak = self.read_single_stat("memory.peak").await?;
        stats.swap = self.read_single_stat("memory.swap.current").await?;
        stats.swap_peak = self.read_single_stat("memory.swap.peak").await?;
        Ok(stats)
    }

    pub async fn read_io_stats(&self) -> Result<IOStats, super::Error> {
        let mut stats = IOStats::default();
        let raw_stats = tokio::fs::read_to_string(self.path.join("io.stat"))
            .await
            .context(super::IOSnafu)?;

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

    pub async fn read_task_stats(&self) -> Result<TaskStats, super::Error> {
        let mut stats = TaskStats::default();
        stats.count = self.read_single_stat("pids.current").await?;
        Ok(stats)
    }
}

impl Display for CGroup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.path.fmt(f)
    }
}
