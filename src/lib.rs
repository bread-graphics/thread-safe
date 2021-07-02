// MIT/Apache2 License

//! Let's say you have some thread-unsafe data. For whatever reason, it can't be used outside of the thread it
//! originated in. This thread-unsafe data is a component of a larger data struct that does need to be sent
//! around between other threads.
//!
//! The `ThreadSafe` contains data that can only be utilized in the thread it was created in. When a reference
//! is attempted to be acquired to the interior data, it checks for the current thread it comes from.
//!
//! # [`ThreadKey`]
//!
//! The `ThreadKey` is a wrapper around `ThreadId`, but `!Send`. This allows one to certify that the current
//! thread has the given `ThreadId`, without having to go through `thread::current().id()`.

use std::{
    error::Error,
    fmt,
    marker::PhantomData,
    mem::{self, ManuallyDrop},
    thread::{self, ThreadId},
};

/// The whole point.
///
/// This structure wraps around thread-unsafe data and only allows access if it comes from the thread that the
/// data originated from. This allows thread-unsafe data to be used in thread-safe structures, as long as
/// the data is only used from the originating thread.
///
/// # Panics
///
/// If the `ThreadSafe` is dropped in a foreign thread, it will panic. This is because running the drop handle
/// for the inner data is considered to be using it in a thread-unsafe context.
pub struct ThreadSafe<T: ?Sized> {
    // thread that we originated in
    origin_thread: ThreadId,
    // whether or not we need to elide the drop check
    handle_drop: bool,
    // inner object
    inner: ManuallyDrop<T>,
}

impl<T: Default> Default for ThreadSafe<T> {
    #[inline]
    fn default() -> Self {
        Self {
            inner: ManuallyDrop::new(T::default()),
            handle_drop: mem::needs_drop::<T>(),
            origin_thread: thread::current().id(),
        }
    }
}

impl<T: fmt::Debug + ?Sized> fmt::Debug for ThreadSafe<T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.origin_thread == thread::current().id() {
            // SAFETY: self.inner can be accessed since we are on the origin thread
            fmt::Debug::fmt(&self.inner, f)
        } else {
            f.write_str("<not in origin thread>")
        }
    }
}

// SAFETY: we check each and every use of "inner" in the below functions. Using "inner" is considered unsafe.
unsafe impl<T> Send for ThreadSafe<T> {}
unsafe impl<T> Sync for ThreadSafe<T> {}

impl<T> ThreadSafe<T> {
    /// Create a new instance of a `ThreadSafe`.
    #[inline]
    pub fn new(inner: T) -> ThreadSafe<T> {
        ThreadSafe {
            origin_thread: thread::current().id(),
            handle_drop: mem::needs_drop::<T>(),
            inner: ManuallyDrop::new(inner),
        }
    }

    /// Attempt to convert to the inner type. This errors if it is not in the origin thread.
    #[inline]
    pub fn try_into_inner(self) -> Result<T, ThreadSafe<T>> {
        self.try_into_inner_with_key(ThreadKey::get())
    }

    /// Attempt to convert to the inner type, using a thread key.
    #[inline]
    pub fn try_into_inner_with_key(mut self, key: ThreadKey) -> Result<T, ThreadSafe<T>> {
        if self.origin_thread == key.id() {
            // SAFETY: "inner" can be used since we are in the origin thread
            //         we can take() because we delete the original right after
            let inner = unsafe { ManuallyDrop::take(&mut self.inner) };
            // SAFETY: suppress the dropper on this object
            mem::forget(self);
            Ok(inner)
        } else {
            Err(self)
        }
    }

    /// Attempt to convert to the inner type. This panics if it is not in the origin thread.
    #[inline]
    pub fn into_inner(self) -> T {
        match self.try_into_inner() {
            Ok(i) => i,
            Err(_) => panic!("Attempted to use a ThreadSafe outside of its origin thread"),
        }
    }

    /// Attempt to convert to the inner type, using a thread key.
    #[inline]
    pub fn into_inner_with_key(self, key: ThreadKey) -> T {
        match self.try_into_inner_with_key(key) {
            Ok(i) => i,
            Err(_) => panic!("Attempted to use a ThreadSafe outside of its origin thread"),
        }
    }
}

impl<T: ?Sized> ThreadSafe<T> {
    /// Try to get a reference to the inner type. This errors if it is not in the origin thread.
    #[inline]
    pub fn try_get_ref(&self) -> Result<&T, NotInOriginThread> {
        self.try_get_ref_with_key(ThreadKey::get())
    }

    /// Try to get a reference to the inner type, using a thread key.
    #[inline]
    pub fn try_get_ref_with_key(&self, key: ThreadKey) -> Result<&T, NotInOriginThread> {
        if self.origin_thread == key.id() {
            // SAFETY: "inner" can be used since we are in the origin thread
            //         it is unlikely that &T can be sent to another thread
            Ok(&self.inner)
        } else {
            Err(NotInOriginThread)
        }
    }

    /// Get a reference to the inner type. This panics if it is not called in the origin thread.
    #[inline]
    pub fn get_ref(&self) -> &T {
        match self.try_get_ref() {
            Ok(i) => i,
            Err(NotInOriginThread) => {
                panic!("Attempted to use a ThreadSafe outside of its origin thread")
            }
        }
    }

    /// Get a reference to the inner type, using a thread key.
    #[inline]
    pub fn get_ref_with_key(&self, key: ThreadKey) -> &T {
        match self.try_get_ref_with_key(key) {
            Ok(i) => i,
            Err(NotInOriginThread) => {
                panic!("Attempted to use a ThreadSafe outside of its origin thread")
            }
        }
    }

    /// Try to get a mutable reference to the inner type. This errors if it is not in the origin thread.
    #[inline]
    pub fn try_get_mut(&mut self) -> Result<&mut T, NotInOriginThread> {
        self.try_get_mut_with_key(ThreadKey::get())
    }

    /// Try to get a mutable reference to the inner type, using a thread key.
    #[inline]
    pub fn try_get_mut_with_key(&mut self, key: ThreadKey) -> Result<&mut T, NotInOriginThread> {
        if self.origin_thread == key.id() {
            // SAFETY: "inner" can be used since we are in the origin thread
            //         it is unlikely that &mut T can be sent to another thread
            Ok(&mut self.inner)
        } else {
            Err(NotInOriginThread)
        }
    }

    /// Get a mutable reference to the inner type. This panics if it is not called in the origin thread.
    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        match self.try_get_mut() {
            Ok(i) => i,
            Err(NotInOriginThread) => {
                panic!("Attempted to use a ThreadSafe outside of its origin thread")
            }
        }
    }

    /// Get a mutable reference to the inner type, using a thread key.
    #[inline]
    pub fn get_mut_with_key(&mut self, key: ThreadKey) -> &mut T {
        match self.try_get_mut_with_key(key) {
            Ok(i) => i,
            Err(NotInOriginThread) => {
                panic!("Attempted to use a ThreadSafe outside of its origin thread")
            }
        }
    }
}

impl<T: Clone> ThreadSafe<T> {
    /// Try to clone this value. This errors if we are not in the origin thread.
    #[inline]
    pub fn try_clone(&self) -> Result<ThreadSafe<T>, NotInOriginThread> {
        self.try_clone_with_key(ThreadKey::get())
    }

    /// Try to clone this value, using a thread key.
    #[inline]
    pub fn try_clone_with_key(&self, key: ThreadKey) -> Result<ThreadSafe<T>, NotInOriginThread> {
        match self.try_get_ref_with_key(key) {
            Ok(r) => Ok(ThreadSafe {
                inner: ManuallyDrop::new(r.clone()),
                handle_drop: self.handle_drop,
                origin_thread: self.origin_thread,
            }),
            Err(NotInOriginThread) => Err(NotInOriginThread),
        }
    }

    /// Clone this value, using a thread key.
    #[inline]
    pub fn clone_with_key(&self, key: ThreadKey) -> ThreadSafe<T> {
        ThreadSafe {
            inner: ManuallyDrop::new(self.get_ref_with_key(key).clone()),
            handle_drop: self.handle_drop,
            origin_thread: self.origin_thread,
        }
    }
}

impl<T: Clone> Clone for ThreadSafe<T> {
    /// Clone this value. This panics if it takes place outside of the origin thread.
    #[inline]
    fn clone(&self) -> ThreadSafe<T> {
        self.clone_with_key(ThreadKey::get())
    }
}

impl<T: ?Sized> Drop for ThreadSafe<T> {
    #[inline]
    fn drop(&mut self) {
        // SAFETY: handle_drop is only turned on if the internal type is needs_drop() in some way
        if self.handle_drop && self.origin_thread != thread::current().id() {
            // SAFETY: we cannot allow the type to be dropped, as this is thread unsafe
            panic!("Attempted to drop ThreadSafe<_> outside of its origin thread");
        } else {
            // SAFETY: since we are dropping the outer struct, and we're in the origin thread, we can drop the
            //         inner object
            unsafe { ManuallyDrop::drop(&mut self.inner) };
        }
    }
}

/// A `ThreadId` that is guaranteed to refer to the current thread, since this is `!Send`.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct ThreadKey {
    id: ThreadId,
    // ensure this is !Send and !Sync
    _phantom: PhantomData<*const ThreadId>,
}

impl Default for ThreadKey {
    #[inline]
    fn default() -> Self {
        Self::get()
    }
}

impl ThreadKey {
    /// Create a new `ThreadKey` based on the current thread.
    #[inline]
    pub fn get() -> Self {
        Self {
            id: thread::current().id(),
            _phantom: PhantomData,
        }
    }

    /// Create a new `ThreadKey` using a `ThreadId`.
    ///
    /// # Safety
    ///
    /// If this `ThreadKey` is ever used, it can only be used in the thread that the thread id refers to.
    #[inline]
    pub unsafe fn new(id: ThreadId) -> Self {
        Self {
            id,
            _phantom: PhantomData,
        }
    }

    /// Get the `ThreadId` for this `ThreadKey`.
    #[inline]
    pub fn id(self) -> ThreadId {
        self.id
    }
}

impl From<ThreadKey> for ThreadId {
    #[inline]
    fn from(k: ThreadKey) -> ThreadId {
        k.id
    }
}

/// Error type for "we are not in the current thread".
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NotInOriginThread;

impl fmt::Display for NotInOriginThread {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Attempted to use ThreadSafe<_> outside of its origin thread")
    }
}

impl Error for NotInOriginThread {}
