use super::constants;
use std::{
    fs, path::{Path, PathBuf}, str::FromStr
};

pub struct Monitor {
    path: Path,
}

impl Monitor {
    pub fn get_stats(&self) {
        let p = self
            .path
            .join(PathBuf::from_str(constants::CGROUP_ROOT).unwrap());

        p.read_dir()
    }
}
