//! Thread ownership metadata for callbacks.

/// Legal execution thread for a callback.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallbackThread {
    /// Main application thread.
    Main,
    /// Render thread.
    Render,
    /// Worker thread pool.
    Worker,
}

/// Callback paired with its legal execution thread.
pub struct ThreadBoundCallback<T> {
    thread: CallbackThread,
    callback: Box<dyn FnMut(T) + Send + 'static>,
}

impl<T> ThreadBoundCallback<T> {
    /// Creates a thread-bound callback.
    pub fn new(thread: CallbackThread, callback: impl FnMut(T) + Send + 'static) -> Self {
        Self {
            thread,
            callback: Box::new(callback),
        }
    }

    /// Returns the legal execution thread.
    pub const fn thread(&self) -> CallbackThread {
        self.thread
    }

    /// Invokes the callback. The caller is responsible for enforcing the thread contract.
    pub fn invoke(&mut self, value: T) {
        (self.callback)(value);
    }
}
