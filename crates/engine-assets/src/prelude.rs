pub(crate) use std::{
    collections::{BTreeSet, HashMap, HashSet, VecDeque},
    fmt, fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    time::{SystemTime, UNIX_EPOCH},
};

#[cfg(any(feature = "importers", feature = "watch"))]
pub(crate) use std::sync::mpsc::Receiver;
#[cfg(feature = "importers")]
pub(crate) use std::{io::Read, sync::mpsc::Sender};

pub(crate) use engine_core::{
    AssetId, EngineError, EngineResult, Handle, HandleAllocator, ResourceId, TaskPriority,
    shared_task_runtime,
};
pub(crate) use serde::{Deserialize, Serialize};
