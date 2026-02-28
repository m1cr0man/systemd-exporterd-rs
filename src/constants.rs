use std::{collections::HashMap, sync::Arc};

use tokio::sync::RwLock;

use crate::service;

pub type UnitMap<'a> = Arc<RwLock<HashMap<String, service::Unit<'a>>>>;
