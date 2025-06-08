pub struct SystemdExporter {}

impl SystemdExporter {
    pub fn new() -> Self {
        Self {}
    }
}

impl From<super::Config> for SystemdExporter {
    fn from(value: super::Config) -> Self {
        Self {}
    }
}
