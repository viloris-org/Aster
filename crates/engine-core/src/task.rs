//! Engine task runtime for CPU work that should not block frame-owned threads.

use std::collections::VecDeque;
use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock, mpsc};
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// Priority bucket used by the engine task runtime.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum TaskPriority {
    /// Latency-sensitive work that should be picked before normal tasks.
    High,
    /// Default CPU work.
    Normal,
    /// Background work that should not compete with frame-critical tasks.
    Background,
}

impl TaskPriority {
    const COUNT: usize = 3;

    const fn queue_index(self) -> usize {
        match self {
            Self::High => 0,
            Self::Normal => 1,
            Self::Background => 2,
        }
    }
}

/// Snapshot of task runtime queue and execution counters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TaskRuntimeStats {
    /// Number of worker threads owned by the runtime.
    pub worker_count: usize,
    /// Number of high-priority tasks waiting in the queue.
    pub queued_high: usize,
    /// Number of normal-priority tasks waiting in the queue.
    pub queued_normal: usize,
    /// Number of background-priority tasks waiting in the queue.
    pub queued_background: usize,
    /// Number of tasks currently running on worker threads.
    pub active: usize,
    /// Number of tasks submitted since runtime creation.
    pub submitted: u64,
    /// Number of tasks that returned a value successfully.
    pub completed: u64,
    /// Number of tasks whose body panicked.
    pub panicked: u64,
}

impl TaskRuntimeStats {
    /// Returns the total number of queued tasks.
    pub const fn queued_total(&self) -> usize {
        self.queued_high + self.queued_normal + self.queued_background
    }

    /// Returns the total number of finished tasks, including panics.
    pub const fn finished(&self) -> u64 {
        self.completed + self.panicked
    }
}

/// Configuration for an [`EngineTaskRuntime`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskRuntimeConfig {
    /// Human-readable worker thread name prefix.
    pub thread_name: String,
    /// Number of worker threads to create.
    pub worker_count: usize,
}

impl TaskRuntimeConfig {
    /// Creates a task runtime configuration with the requested worker count.
    pub fn new(worker_count: usize) -> Self {
        Self {
            thread_name: "varg-task".to_owned(),
            worker_count: worker_count.max(1),
        }
    }
}

impl Default for TaskRuntimeConfig {
    fn default() -> Self {
        let worker_count = thread::available_parallelism()
            .map(usize::from)
            .unwrap_or(1)
            .saturating_sub(1)
            .max(1);
        Self::new(worker_count)
    }
}

/// Error returned when waiting for a task result.
#[derive(Debug, thiserror::Error)]
pub enum TaskJoinError {
    /// The task runtime stopped before returning a value.
    #[error("task `{debug_name}` was canceled before completion")]
    Canceled {
        /// Debug name supplied when the task was spawned.
        debug_name: String,
    },

    /// The task body panicked.
    #[error("task `{debug_name}` panicked")]
    Panicked {
        /// Debug name supplied when the task was spawned.
        debug_name: String,
    },
}

enum TaskOutcome<T> {
    Completed(T),
    Panicked,
}

/// Handle to a spawned engine task.
pub struct TaskHandle<T> {
    debug_name: String,
    completed: Arc<AtomicBool>,
    result_rx: mpsc::Receiver<TaskOutcome<T>>,
}

impl<T> TaskHandle<T> {
    /// Returns the task debug name.
    pub fn debug_name(&self) -> &str {
        &self.debug_name
    }

    /// Returns true once the task has finished running.
    pub fn is_finished(&self) -> bool {
        self.completed.load(Ordering::Acquire)
    }

    /// Returns a lightweight prerequisite handle for scheduling dependent tasks.
    pub fn dependency(&self) -> TaskDependency {
        TaskDependency {
            debug_name: self.debug_name.clone(),
            completed: Arc::clone(&self.completed),
        }
    }

    /// Blocks until the task finishes and returns its result.
    pub fn wait(self) -> Result<T, TaskJoinError> {
        match self.result_rx.recv() {
            Ok(TaskOutcome::Completed(value)) => Ok(value),
            Ok(TaskOutcome::Panicked) => Err(TaskJoinError::Panicked {
                debug_name: self.debug_name,
            }),
            Err(_) => Err(TaskJoinError::Canceled {
                debug_name: self.debug_name,
            }),
        }
    }

    fn finish_from_outcome(self, outcome: TaskOutcome<T>) -> Result<T, TaskJoinError> {
        match outcome {
            TaskOutcome::Completed(value) => Ok(value),
            TaskOutcome::Panicked => Err(TaskJoinError::Panicked {
                debug_name: self.debug_name,
            }),
        }
    }

    fn canceled(self) -> TaskJoinError {
        TaskJoinError::Canceled {
            debug_name: self.debug_name,
        }
    }
}

/// Lightweight completion token used to defer a task until prerequisites finish.
///
/// A dependency only tracks completion. It does not transfer the prerequisite's
/// return value or panic state to the dependent task.
#[derive(Clone, Debug)]
pub struct TaskDependency {
    debug_name: String,
    completed: Arc<AtomicBool>,
}

impl TaskDependency {
    /// Returns the task debug name that produced this dependency.
    pub fn debug_name(&self) -> &str {
        &self.debug_name
    }

    /// Returns true once the prerequisite task has finished running.
    pub fn is_satisfied(&self) -> bool {
        self.completed.load(Ordering::Acquire)
    }
}

/// Manually triggered completion event that can be used as a task prerequisite.
///
/// This is useful when a dependency is owned by an external system rather than
/// another engine task, such as file watcher input or a render-thread handoff.
#[derive(Clone)]
pub struct TaskEvent {
    debug_name: String,
    completed: Arc<AtomicBool>,
    state: Arc<SharedTaskState>,
}

impl TaskEvent {
    /// Returns the event debug name.
    pub fn debug_name(&self) -> &str {
        &self.debug_name
    }

    /// Returns true once this event has been triggered.
    pub fn is_triggered(&self) -> bool {
        self.completed.load(Ordering::Acquire)
    }

    /// Returns a lightweight prerequisite handle for scheduling dependent tasks.
    pub fn dependency(&self) -> TaskDependency {
        TaskDependency {
            debug_name: self.debug_name.clone(),
            completed: Arc::clone(&self.completed),
        }
    }

    /// Triggers the event and wakes workers that may now have ready tasks.
    ///
    /// Returns true when this call changed the event from unsignaled to
    /// signaled, or false when it had already been triggered.
    pub fn trigger(&self) -> bool {
        let was_triggered = self.completed.swap(true, Ordering::AcqRel);
        if !was_triggered {
            self.state.wake_workers.notify_all();
        }
        !was_triggered
    }
}

/// Snapshot of a [`TaskConcurrencyLimiter`].
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TaskConcurrencyLimiterStats {
    /// Maximum number of limiter tasks allowed to run at once.
    pub max_concurrency: usize,
    /// Number of limiter tasks waiting for a free concurrency slot.
    pub queued: usize,
    /// Number of limiter tasks currently running.
    pub active: usize,
    /// Number of limiter tasks pushed since creation.
    pub submitted: u64,
    /// Number of limiter tasks that returned successfully.
    pub completed: u64,
    /// Number of limiter tasks whose body panicked.
    pub panicked: u64,
}

impl TaskConcurrencyLimiterStats {
    /// Returns the total number of finished limiter tasks, including panics.
    pub const fn finished(&self) -> u64 {
        self.completed + self.panicked
    }

    /// Returns the total number of queued and active limiter tasks.
    pub fn pending(&self) -> u64 {
        self.submitted.saturating_sub(self.finished())
    }
}

/// Limits how many tasks from a batch can run concurrently on an engine runtime.
///
/// This is intended for bulk background work such as asset imports or analysis
/// passes. Queued items are submitted to the shared worker pool only when a
/// limiter slot is available, so they do not occupy worker threads while
/// waiting for capacity.
pub struct TaskConcurrencyLimiter {
    state: Arc<TaskConcurrencyLimiterState>,
}

impl TaskConcurrencyLimiter {
    /// Pushes one task into the limiter queue.
    ///
    /// The `slot` argument passed to the task is unique among concurrently
    /// running tasks from this limiter and is in `0..max_concurrency`.
    pub fn push<F>(&self, debug_name: impl Into<String>, task: F)
    where
        F: FnOnce(usize) + Send + 'static,
    {
        {
            let mut inner = self
                .state
                .inner
                .lock()
                .expect("task concurrency limiter mutex poisoned");
            inner.submitted += 1;
            inner.queue.push_back(TaskConcurrencyLimiterJob {
                debug_name: debug_name.into(),
                run: Box::new(task),
            });
        }
        schedule_limiter_jobs(&self.state);
    }

    /// Waits until all currently submitted limiter tasks have finished.
    ///
    /// While waiting, this method executes ready work from the same engine
    /// runtime so a waiting thread can help drain the task graph.
    pub fn wait(&self) {
        loop {
            {
                let inner = self
                    .state
                    .inner
                    .lock()
                    .expect("task concurrency limiter mutex poisoned");
                if inner.pending() == 0 {
                    return;
                }
            }

            if let Some(job) = pop_ready_job(&self.state.runtime_state) {
                job.run();
                continue;
            }

            let inner = self
                .state
                .inner
                .lock()
                .expect("task concurrency limiter mutex poisoned");
            let _ = self
                .state
                .finished
                .wait_timeout(inner, Duration::from_millis(1))
                .expect("task concurrency limiter mutex poisoned");
        }
    }

    /// Returns a point-in-time snapshot of limiter counters.
    pub fn stats(&self) -> TaskConcurrencyLimiterStats {
        let inner = self
            .state
            .inner
            .lock()
            .expect("task concurrency limiter mutex poisoned");
        TaskConcurrencyLimiterStats {
            max_concurrency: self.state.max_concurrency,
            queued: inner.queue.len(),
            active: inner.active,
            submitted: inner.submitted,
            completed: inner.completed,
            panicked: inner.panicked,
        }
    }
}

impl fmt::Debug for TaskConcurrencyLimiter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TaskConcurrencyLimiter")
            .field("stats", &self.stats())
            .finish()
    }
}

struct TaskConcurrencyLimiterState {
    runtime_state: Arc<SharedTaskState>,
    priority: TaskPriority,
    max_concurrency: usize,
    inner: Mutex<TaskConcurrencyLimiterInner>,
    finished: Condvar,
}

struct TaskConcurrencyLimiterInner {
    queue: VecDeque<TaskConcurrencyLimiterJob>,
    free_slots: Vec<usize>,
    active: usize,
    submitted: u64,
    completed: u64,
    panicked: u64,
}

impl TaskConcurrencyLimiterInner {
    fn new(max_concurrency: usize) -> Self {
        Self {
            queue: VecDeque::new(),
            free_slots: (0..max_concurrency).rev().collect(),
            active: 0,
            submitted: 0,
            completed: 0,
            panicked: 0,
        }
    }

    fn pending(&self) -> u64 {
        self.submitted
            .saturating_sub(self.completed.saturating_add(self.panicked))
    }
}

struct TaskConcurrencyLimiterJob {
    debug_name: String,
    run: Box<dyn FnOnce(usize) + Send + 'static>,
}

impl fmt::Debug for TaskEvent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TaskEvent")
            .field("debug_name", &self.debug_name)
            .field("is_triggered", &self.is_triggered())
            .finish_non_exhaustive()
    }
}

impl<T> fmt::Debug for TaskHandle<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TaskHandle")
            .field("debug_name", &self.debug_name)
            .field("is_finished", &self.is_finished())
            .finish_non_exhaustive()
    }
}

struct TaskJob {
    priority: TaskPriority,
    prerequisites: Vec<TaskDependency>,
    run: Option<Box<dyn FnOnce() + Send + 'static>>,
}

impl TaskJob {
    fn is_ready(&self) -> bool {
        self.prerequisites.iter().all(TaskDependency::is_satisfied)
    }

    fn run(mut self) {
        if let Some(run) = self.run.take() {
            run();
        }
    }
}

#[derive(Default)]
struct TaskQueues {
    queues: [VecDeque<TaskJob>; TaskPriority::COUNT],
    shutdown: bool,
}

impl TaskQueues {
    fn push(&mut self, job: TaskJob) {
        self.queues[job.priority.queue_index()].push_back(job);
    }

    fn pop_ready(&mut self) -> Option<TaskJob> {
        self.queues.iter_mut().find_map(|queue| {
            let index = queue.iter().position(TaskJob::is_ready)?;
            queue.remove(index)
        })
    }

    fn len(&self, priority: TaskPriority) -> usize {
        self.queues[priority.queue_index()].len()
    }
}

#[derive(Default)]
struct TaskCounters {
    active: AtomicUsize,
    submitted: AtomicU64,
    completed: AtomicU64,
    panicked: AtomicU64,
}

struct SharedTaskState {
    queues: Mutex<TaskQueues>,
    wake_workers: Condvar,
    counters: Arc<TaskCounters>,
}

/// Small engine-owned CPU task runtime.
///
/// This runtime is intentionally separate from render, window, and editor main
/// threads. It is meant for short to medium CPU work such as preparation,
/// import, compilation, and background analysis jobs.
pub struct EngineTaskRuntime {
    state: Arc<SharedTaskState>,
    workers: Vec<JoinHandle<()>>,
}

impl EngineTaskRuntime {
    /// Creates a runtime with the default worker count.
    pub fn new() -> Self {
        Self::with_config(TaskRuntimeConfig::default())
    }

    /// Creates a runtime using the supplied configuration.
    pub fn with_config(config: TaskRuntimeConfig) -> Self {
        let state = Arc::new(SharedTaskState {
            queues: Mutex::new(TaskQueues::default()),
            wake_workers: Condvar::new(),
            counters: Arc::new(TaskCounters::default()),
        });
        let mut workers = Vec::with_capacity(config.worker_count);

        for index in 0..config.worker_count {
            let worker_state = Arc::clone(&state);
            let thread_name = format!("{}-{index}", config.thread_name);
            let worker = thread::Builder::new()
                .name(thread_name)
                .spawn(move || worker_loop(worker_state))
                .expect("spawn engine task worker");
            workers.push(worker);
        }

        Self { state, workers }
    }

    /// Returns the number of worker threads in this runtime.
    pub fn worker_count(&self) -> usize {
        self.workers.len()
    }

    /// Returns a point-in-time snapshot of queue and execution counters.
    pub fn stats(&self) -> TaskRuntimeStats {
        let queues = self
            .state
            .queues
            .lock()
            .expect("engine task queue mutex poisoned");
        TaskRuntimeStats {
            worker_count: self.worker_count(),
            queued_high: queues.len(TaskPriority::High),
            queued_normal: queues.len(TaskPriority::Normal),
            queued_background: queues.len(TaskPriority::Background),
            active: self.state.counters.active.load(Ordering::Acquire),
            submitted: self.state.counters.submitted.load(Ordering::Acquire),
            completed: self.state.counters.completed.load(Ordering::Acquire),
            panicked: self.state.counters.panicked.load(Ordering::Acquire),
        }
    }

    /// Creates a concurrency limiter backed by this runtime.
    pub fn concurrency_limiter(
        &self,
        max_concurrency: usize,
        priority: TaskPriority,
    ) -> TaskConcurrencyLimiter {
        TaskConcurrencyLimiter {
            state: Arc::new(TaskConcurrencyLimiterState {
                runtime_state: Arc::clone(&self.state),
                priority,
                max_concurrency: max_concurrency.max(1),
                inner: Mutex::new(TaskConcurrencyLimiterInner::new(max_concurrency.max(1))),
                finished: Condvar::new(),
            }),
        }
    }

    /// Spawns a task and returns a typed handle for waiting on the result.
    pub fn spawn<T, F>(
        &self,
        debug_name: impl Into<String>,
        priority: TaskPriority,
        task: F,
    ) -> TaskHandle<T>
    where
        T: Send + 'static,
        F: FnOnce() -> T + Send + 'static,
    {
        self.spawn_after(debug_name, priority, [], task)
    }

    /// Spawns a task after all prerequisite dependencies have finished.
    ///
    /// Prerequisites affect scheduling only: the dependent task runs after each
    /// dependency is complete, even if a prerequisite panicked.
    pub fn spawn_after<T, F, I>(
        &self,
        debug_name: impl Into<String>,
        priority: TaskPriority,
        prerequisites: I,
        task: F,
    ) -> TaskHandle<T>
    where
        T: Send + 'static,
        F: FnOnce() -> T + Send + 'static,
        I: IntoIterator<Item = TaskDependency>,
    {
        let debug_name = debug_name.into();
        let handle_debug_name = debug_name.clone();
        let (result_tx, result_rx) = mpsc::channel();
        let completed = Arc::new(AtomicBool::new(false));
        let completed_for_task = Arc::clone(&completed);
        let state = Arc::clone(&self.state);
        let counters = Arc::clone(&self.state.counters);
        counters.submitted.fetch_add(1, Ordering::AcqRel);

        let run = Box::new(move || {
            counters.active.fetch_add(1, Ordering::AcqRel);
            let outcome = match catch_unwind(AssertUnwindSafe(task)) {
                Ok(value) => {
                    counters.completed.fetch_add(1, Ordering::AcqRel);
                    TaskOutcome::Completed(value)
                }
                Err(_) => {
                    counters.panicked.fetch_add(1, Ordering::AcqRel);
                    TaskOutcome::Panicked
                }
            };
            completed_for_task.store(true, Ordering::Release);
            counters.active.fetch_sub(1, Ordering::AcqRel);
            let _ = result_tx.send(outcome);
            state.wake_workers.notify_all();
        });

        let mut queues = self
            .state
            .queues
            .lock()
            .expect("engine task queue mutex poisoned");
        queues.push(TaskJob {
            priority,
            prerequisites: prerequisites.into_iter().collect(),
            run: Some(run),
        });
        drop(queues);
        self.state.wake_workers.notify_one();

        TaskHandle {
            debug_name: handle_debug_name,
            completed,
            result_rx,
        }
    }

    /// Creates a manually triggered event that can be used as a prerequisite.
    pub fn event(&self, debug_name: impl Into<String>) -> TaskEvent {
        TaskEvent {
            debug_name: debug_name.into(),
            completed: Arc::new(AtomicBool::new(false)),
            state: Arc::clone(&self.state),
        }
    }

    /// Spawns a no-op barrier task that completes after all prerequisites finish.
    pub fn spawn_barrier<I>(
        &self,
        debug_name: impl Into<String>,
        priority: TaskPriority,
        prerequisites: I,
    ) -> TaskHandle<()>
    where
        I: IntoIterator<Item = TaskDependency>,
    {
        self.spawn_after(debug_name, priority, prerequisites, || {})
    }

    /// Waits for a homogeneous collection of task handles and returns their results.
    pub fn wait_all<T, I>(&self, handles: I) -> Result<Vec<T>, TaskJoinError>
    where
        I: IntoIterator<Item = TaskHandle<T>>,
    {
        handles
            .into_iter()
            .map(|handle| self.wait_for(handle))
            .collect()
    }

    /// Waits for a task while executing other ready tasks from this runtime.
    ///
    /// This mirrors the practical behavior of task systems that avoid parking a
    /// worker thread when it can make forward progress on the same task graph.
    /// Use this when waiting from code that may itself run inside the task
    /// runtime.
    pub fn wait_for<T>(&self, handle: TaskHandle<T>) -> Result<T, TaskJoinError> {
        loop {
            match handle.result_rx.try_recv() {
                Ok(outcome) => return handle.finish_from_outcome(outcome),
                Err(mpsc::TryRecvError::Disconnected) => return Err(handle.canceled()),
                Err(mpsc::TryRecvError::Empty) => {}
            }

            if let Some(job) = self.pop_ready_job() {
                job.run();
                continue;
            }

            match handle.result_rx.recv_timeout(Duration::from_millis(1)) {
                Ok(outcome) => return handle.finish_from_outcome(outcome),
                Err(mpsc::RecvTimeoutError::Disconnected) => return Err(handle.canceled()),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
        }
    }

    /// Runs a range in parallel using this runtime and waits for completion.
    ///
    /// The caller executes one chunk inline while the remaining chunks are
    /// scheduled onto worker threads. This keeps the helper usable from a worker
    /// task without requiring that worker to block all available progress.
    pub fn parallel_for<F>(
        &self,
        debug_name: impl Into<String>,
        priority: TaskPriority,
        range: std::ops::Range<usize>,
        min_chunk_size: usize,
        body: F,
    ) -> Result<(), TaskJoinError>
    where
        F: Fn(usize) + Send + Sync + 'static,
    {
        let debug_name = debug_name.into();
        let len = range.end.saturating_sub(range.start);
        if len == 0 {
            return Ok(());
        }

        let chunk_size = min_chunk_size.max(1);
        let max_chunks = self.worker_count().max(1);
        let chunk_count = len.div_ceil(chunk_size).min(max_chunks).max(1);
        let actual_chunk_size = len.div_ceil(chunk_count);
        let body = Arc::new(body);
        let mut handles = Vec::with_capacity(chunk_count.saturating_sub(1));

        for chunk_index in 1..chunk_count {
            let start = range.start + chunk_index * actual_chunk_size;
            let end = (start + actual_chunk_size).min(range.end);
            if start >= end {
                continue;
            }

            let task_body = Arc::clone(&body);
            handles.push(self.spawn(
                format!("{debug_name}.chunk-{chunk_index}"),
                priority,
                move || {
                    for index in start..end {
                        task_body(index);
                    }
                },
            ));
        }

        let local_end = (range.start + actual_chunk_size).min(range.end);
        let local_outcome = catch_unwind(AssertUnwindSafe(|| {
            for index in range.start..local_end {
                body(index);
            }
        }));

        let mut first_error = match local_outcome {
            Ok(()) => None,
            Err(_) => Some(TaskJoinError::Panicked {
                debug_name: debug_name.clone(),
            }),
        };

        for handle in handles {
            if let Err(error) = handle.wait() {
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }

        match first_error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    fn pop_ready_job(&self) -> Option<TaskJob> {
        pop_ready_job(&self.state)
    }
}

impl Default for EngineTaskRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for EngineTaskRuntime {
    fn drop(&mut self) {
        let mut queues = self
            .state
            .queues
            .lock()
            .expect("engine task queue mutex poisoned");
        queues.shutdown = true;
        drop(queues);
        self.state.wake_workers.notify_all();

        while let Some(worker) = self.workers.pop() {
            let _ = worker.join();
        }
    }
}

/// Returns the process-wide shared engine task runtime.
///
/// This is suitable for subsystems that need a common background CPU pool and
/// do not own a runtime lifecycle themselves.
pub fn shared_task_runtime() -> &'static EngineTaskRuntime {
    static RUNTIME: OnceLock<EngineTaskRuntime> = OnceLock::new();
    RUNTIME.get_or_init(EngineTaskRuntime::new)
}

fn worker_loop(state: Arc<SharedTaskState>) {
    loop {
        let job = {
            let mut queues = state
                .queues
                .lock()
                .expect("engine task queue mutex poisoned");
            loop {
                if let Some(job) = queues.pop_ready() {
                    break job;
                }
                if queues.shutdown {
                    return;
                }
                queues = state
                    .wake_workers
                    .wait(queues)
                    .expect("engine task queue mutex poisoned");
            }
        };
        job.run();
    }
}

fn submit_detached(
    state: &Arc<SharedTaskState>,
    priority: TaskPriority,
    prerequisites: Vec<TaskDependency>,
    task: impl FnOnce() + Send + 'static,
) {
    state.counters.submitted.fetch_add(1, Ordering::AcqRel);

    let run_state = Arc::clone(state);
    let counters = Arc::clone(&state.counters);
    let run = Box::new(move || {
        counters.active.fetch_add(1, Ordering::AcqRel);
        match catch_unwind(AssertUnwindSafe(task)) {
            Ok(()) => {
                counters.completed.fetch_add(1, Ordering::AcqRel);
            }
            Err(_) => {
                counters.panicked.fetch_add(1, Ordering::AcqRel);
            }
        }
        counters.active.fetch_sub(1, Ordering::AcqRel);
        run_state.wake_workers.notify_all();
    });

    let mut queues = state
        .queues
        .lock()
        .expect("engine task queue mutex poisoned");
    queues.push(TaskJob {
        priority,
        prerequisites,
        run: Some(run),
    });
    drop(queues);
    state.wake_workers.notify_one();
}

fn pop_ready_job(state: &Arc<SharedTaskState>) -> Option<TaskJob> {
    let mut queues = state
        .queues
        .lock()
        .expect("engine task queue mutex poisoned");
    queues.pop_ready()
}

fn schedule_limiter_jobs(state: &Arc<TaskConcurrencyLimiterState>) {
    loop {
        let (slot, job) = {
            let mut inner = state
                .inner
                .lock()
                .expect("task concurrency limiter mutex poisoned");
            let Some(slot) = inner.free_slots.pop() else {
                return;
            };
            let Some(job) = inner.queue.pop_front() else {
                inner.free_slots.push(slot);
                return;
            };
            inner.active += 1;
            (slot, job)
        };

        let limiter_state = Arc::clone(state);
        submit_detached(
            &state.runtime_state,
            state.priority,
            Vec::new(),
            move || run_limiter_job(limiter_state, slot, job),
        );
    }
}

fn run_limiter_job(
    state: Arc<TaskConcurrencyLimiterState>,
    slot: usize,
    job: TaskConcurrencyLimiterJob,
) {
    let TaskConcurrencyLimiterJob { debug_name, run } = job;
    let panicked = catch_unwind(AssertUnwindSafe(|| run(slot))).is_err();
    {
        let mut inner = state
            .inner
            .lock()
            .expect("task concurrency limiter mutex poisoned");
        inner.active = inner.active.saturating_sub(1);
        inner.free_slots.push(slot);
        if panicked {
            inner.panicked += 1;
        } else {
            inner.completed += 1;
        }
    }
    state.finished.notify_all();
    schedule_limiter_jobs(&state);
    if panicked {
        panic!("task concurrency limiter job `{debug_name}` panicked");
    }
}

#[cfg(test)]
mod tests {
    use super::{EngineTaskRuntime, TaskJoinError, TaskPriority, TaskRuntimeConfig};

    #[test]
    fn task_runtime_returns_spawned_result() {
        let runtime = EngineTaskRuntime::with_config(TaskRuntimeConfig::new(1));
        let handle = runtime.spawn("answer", TaskPriority::Normal, || 42);

        assert_eq!(handle.wait().unwrap(), 42);
    }

    #[test]
    fn task_runtime_runs_high_priority_before_background() {
        let runtime = EngineTaskRuntime::with_config(TaskRuntimeConfig::new(1));
        let (resume_tx, resume_rx) = std::sync::mpsc::channel();
        let (order_tx, order_rx) = std::sync::mpsc::channel();
        let first = runtime.spawn("gate", TaskPriority::Normal, move || {
            resume_rx.recv().unwrap();
        });

        let low_order_tx = order_tx.clone();
        let low = runtime.spawn("background", TaskPriority::Background, move || {
            low_order_tx.send("background").unwrap();
        });
        let high = runtime.spawn("high", TaskPriority::High, move || {
            order_tx.send("high").unwrap();
        });

        resume_tx.send(()).unwrap();
        first.wait().unwrap();
        low.wait().unwrap();
        high.wait().unwrap();

        assert_eq!(order_rx.recv().unwrap(), "high");
        assert_eq!(order_rx.recv().unwrap(), "background");
    }

    #[test]
    fn task_handle_reports_finished_state() {
        let runtime = EngineTaskRuntime::with_config(TaskRuntimeConfig::new(1));
        let (resume_tx, resume_rx) = std::sync::mpsc::channel();
        let handle = runtime.spawn("finished", TaskPriority::Normal, move || {
            resume_rx.recv().unwrap();
            "done"
        });

        assert!(!handle.is_finished());
        resume_tx.send(()).unwrap();
        assert_eq!(handle.wait().unwrap(), "done");
    }

    #[test]
    fn task_runtime_reports_panics() {
        let runtime = EngineTaskRuntime::with_config(TaskRuntimeConfig::new(1));
        let handle = runtime.spawn("panic", TaskPriority::Normal, || -> usize {
            panic!("intentional task panic");
        });

        assert!(handle.wait().is_err());
    }

    #[test]
    fn task_runtime_stats_track_queued_active_and_finished_tasks() {
        let runtime = EngineTaskRuntime::with_config(TaskRuntimeConfig::new(1));
        let (resume_tx, resume_rx) = std::sync::mpsc::channel();
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let gate = runtime.spawn("gate", TaskPriority::Normal, move || {
            started_tx.send(()).unwrap();
            resume_rx.recv().unwrap();
        });
        started_rx.recv().unwrap();

        let background = runtime.spawn("background", TaskPriority::Background, || "background");
        let high = runtime.spawn("high", TaskPriority::High, || "high");

        let stats = runtime.stats();
        assert_eq!(stats.worker_count, 1);
        assert_eq!(stats.submitted, 3);
        assert_eq!(stats.active, 1);
        assert_eq!(stats.queued_high, 1);
        assert_eq!(stats.queued_background, 1);
        assert_eq!(stats.queued_total(), 2);
        assert_eq!(stats.finished(), 0);

        resume_tx.send(()).unwrap();
        gate.wait().unwrap();
        assert_eq!(high.wait().unwrap(), "high");
        assert_eq!(background.wait().unwrap(), "background");

        let stats = runtime.stats();
        assert_eq!(stats.active, 0);
        assert_eq!(stats.queued_total(), 0);
        assert_eq!(stats.completed, 3);
        assert_eq!(stats.panicked, 0);
        assert_eq!(stats.finished(), 3);
    }

    #[test]
    fn task_runtime_stats_track_panicked_tasks() {
        let runtime = EngineTaskRuntime::with_config(TaskRuntimeConfig::new(1));
        let handle = runtime.spawn("panic", TaskPriority::Normal, || {
            panic!("intentional task panic");
        });

        assert!(handle.wait().is_err());

        let stats = runtime.stats();
        assert_eq!(stats.submitted, 1);
        assert_eq!(stats.completed, 0);
        assert_eq!(stats.panicked, 1);
        assert_eq!(stats.finished(), 1);
    }

    #[test]
    fn dependent_task_waits_for_prerequisite() {
        let runtime = EngineTaskRuntime::with_config(TaskRuntimeConfig::new(1));
        let (resume_tx, resume_rx) = std::sync::mpsc::channel();
        let (order_tx, order_rx) = std::sync::mpsc::channel();

        let prerequisite = runtime.spawn("prerequisite", TaskPriority::Normal, {
            let order_tx = order_tx.clone();
            move || {
                resume_rx.recv().unwrap();
                order_tx.send("prerequisite").unwrap();
            }
        });
        let dependent = runtime.spawn_after(
            "dependent",
            TaskPriority::High,
            [prerequisite.dependency()],
            move || {
                order_tx.send("dependent").unwrap();
            },
        );

        resume_tx.send(()).unwrap();
        prerequisite.wait().unwrap();
        dependent.wait().unwrap();

        assert_eq!(order_rx.recv().unwrap(), "prerequisite");
        assert_eq!(order_rx.recv().unwrap(), "dependent");
    }

    #[test]
    fn dependent_task_runs_after_panicked_prerequisite_finishes() {
        let runtime = EngineTaskRuntime::with_config(TaskRuntimeConfig::new(1));
        let prerequisite = runtime.spawn("prerequisite", TaskPriority::Normal, || {
            panic!("intentional task panic");
        });
        let dependent = runtime.spawn_after(
            "dependent",
            TaskPriority::Normal,
            [prerequisite.dependency()],
            || 7,
        );

        assert!(prerequisite.wait().is_err());
        assert_eq!(dependent.wait().unwrap(), 7);
    }

    #[test]
    fn task_event_releases_dependent_task_when_triggered() {
        let runtime = EngineTaskRuntime::with_config(TaskRuntimeConfig::new(1));
        let event = runtime.event("external-ready");
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let dependent = runtime.spawn_after(
            "dependent",
            TaskPriority::High,
            [event.dependency()],
            move || done_tx.send("done").unwrap(),
        );

        assert!(done_rx.try_recv().is_err());
        assert!(event.trigger());
        assert!(!event.trigger());
        dependent.wait().unwrap();

        assert_eq!(done_rx.recv().unwrap(), "done");
    }

    #[test]
    fn barrier_runs_after_all_prerequisites() {
        let runtime = EngineTaskRuntime::with_config(TaskRuntimeConfig::new(2));
        let (resume_tx, resume_rx) = std::sync::mpsc::channel();
        let first = runtime.spawn("first", TaskPriority::Normal, || 1);
        let second = runtime.spawn("second", TaskPriority::Normal, move || {
            resume_rx.recv().unwrap();
            2
        });
        let barrier = runtime.spawn_barrier(
            "barrier",
            TaskPriority::High,
            [first.dependency(), second.dependency()],
        );

        assert!(!barrier.is_finished());
        assert_eq!(first.wait().unwrap(), 1);
        assert!(!barrier.is_finished());

        resume_tx.send(()).unwrap();
        assert_eq!(second.wait().unwrap(), 2);
        barrier.wait().unwrap();
    }

    #[test]
    fn wait_all_returns_results_in_handle_order() {
        let runtime = EngineTaskRuntime::with_config(TaskRuntimeConfig::new(2));
        let handles = [
            runtime.spawn("one", TaskPriority::Normal, || 1),
            runtime.spawn("two", TaskPriority::Normal, || 2),
            runtime.spawn("three", TaskPriority::Normal, || 3),
        ];

        assert_eq!(runtime.wait_all(handles).unwrap(), vec![1, 2, 3]);
    }

    #[test]
    fn wait_all_returns_first_join_error() {
        let runtime = EngineTaskRuntime::with_config(TaskRuntimeConfig::new(2));
        let handles = [
            runtime.spawn("panic", TaskPriority::Normal, || -> usize {
                panic!("intentional task panic");
            }),
            runtime.spawn("ok", TaskPriority::Normal, || 2),
        ];

        assert!(matches!(
            runtime.wait_all(handles).unwrap_err(),
            TaskJoinError::Panicked { .. }
        ));
    }

    #[test]
    fn wait_for_executes_ready_work_while_waiting() {
        let runtime = EngineTaskRuntime::with_config(TaskRuntimeConfig::new(1));
        let (resume_tx, resume_rx) = std::sync::mpsc::channel();
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let gate = runtime.spawn("gate", TaskPriority::Normal, move || {
            started_tx.send(()).unwrap();
            resume_rx.recv().unwrap();
        });
        started_rx.recv().unwrap();

        let target = runtime.spawn("target", TaskPriority::Normal, || 9);

        assert_eq!(runtime.wait_for(target).unwrap(), 9);
        resume_tx.send(()).unwrap();
        gate.wait().unwrap();
    }

    #[test]
    fn concurrency_limiter_caps_active_work() {
        let runtime = EngineTaskRuntime::with_config(TaskRuntimeConfig::new(4));
        let limiter = runtime.concurrency_limiter(2, TaskPriority::Normal);
        let active = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let peak = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let slots = std::sync::Arc::new(
            (0..2)
                .map(|_| std::sync::atomic::AtomicUsize::new(0))
                .collect::<Vec<_>>(),
        );

        for index in 0..8 {
            let active = std::sync::Arc::clone(&active);
            let peak = std::sync::Arc::clone(&peak);
            let slots = std::sync::Arc::clone(&slots);
            limiter.push(format!("limited-{index}"), move |slot| {
                assert!(slot < 2);
                slots[slot].fetch_add(1, std::sync::atomic::Ordering::AcqRel);
                let now = active.fetch_add(1, std::sync::atomic::Ordering::AcqRel) + 1;
                peak.fetch_max(now, std::sync::atomic::Ordering::AcqRel);
                std::thread::sleep(std::time::Duration::from_millis(2));
                active.fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
            });
        }

        limiter.wait();

        let stats = limiter.stats();
        assert_eq!(stats.max_concurrency, 2);
        assert_eq!(stats.queued, 0);
        assert_eq!(stats.active, 0);
        assert_eq!(stats.submitted, 8);
        assert_eq!(stats.completed, 8);
        assert_eq!(stats.panicked, 0);
        assert_eq!(stats.pending(), 0);
        assert!(peak.load(std::sync::atomic::Ordering::Acquire) <= 2);
        assert!(
            slots
                .iter()
                .all(|slot| slot.load(std::sync::atomic::Ordering::Acquire) > 0)
        );
    }

    #[test]
    fn concurrency_limiter_records_panics_and_continues() {
        let runtime = EngineTaskRuntime::with_config(TaskRuntimeConfig::new(2));
        let limiter = runtime.concurrency_limiter(1, TaskPriority::Normal);
        let completed = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));

        limiter.push("panic", |_| {
            panic!("intentional limiter panic");
        });
        let completed_for_task = std::sync::Arc::clone(&completed);
        limiter.push("ok", move |_| {
            completed_for_task.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        });

        limiter.wait();

        let stats = limiter.stats();
        assert_eq!(stats.submitted, 2);
        assert_eq!(stats.completed, 1);
        assert_eq!(stats.panicked, 1);
        assert_eq!(stats.pending(), 0);
        assert_eq!(completed.load(std::sync::atomic::Ordering::Acquire), 1);
    }

    #[test]
    fn parallel_for_processes_each_index() {
        let runtime = EngineTaskRuntime::with_config(TaskRuntimeConfig::new(3));
        let values = std::sync::Arc::new(
            (0..16)
                .map(|_| std::sync::atomic::AtomicUsize::new(0))
                .collect::<Vec<_>>(),
        );
        let written = std::sync::Arc::clone(&values);

        runtime
            .parallel_for("write", TaskPriority::Normal, 0..16, 2, move |index| {
                written[index].fetch_add(index + 1, std::sync::atomic::Ordering::AcqRel);
            })
            .unwrap();

        for (index, value) in values.iter().enumerate() {
            assert_eq!(value.load(std::sync::atomic::Ordering::Acquire), index + 1);
        }
    }

    #[test]
    fn parallel_for_reports_inline_panic() {
        let runtime = EngineTaskRuntime::with_config(TaskRuntimeConfig::new(1));
        let error = runtime
            .parallel_for("panic", TaskPriority::Normal, 0..1, 1, |_| {
                panic!("intentional task panic");
            })
            .unwrap_err();

        assert!(matches!(error, TaskJoinError::Panicked { .. }));
    }
}
