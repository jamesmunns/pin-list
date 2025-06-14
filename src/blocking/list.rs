//! The list of a PinList

use core::pin::Pin;

use cordyceps::List;
use mutex::{BlockingMutex, ConstInit, ScopedRawMutex};

use super::node::NodeHeader;

/// An intrusive list of [`Node<T>`]s
///
/// [`Node<T>`]: crate::blocking::node::Node
pub struct PinList<R: ScopedRawMutex, T> {
    pub(crate) inner: BlockingMutex<R, PinListInner<T>>,
}

/// An [`Iterator`] over `&T` nodes of a [`PinList`]
///
/// Obtained by calling [`PinList::with_iter()`].
pub struct Iter<'a, T> {
    iter: cordyceps::list::Iter<'a, NodeHeader<T>>,
}

/// An [`Iterator`] over `Pin<&mut T>` nodes of a [`PinList`]
///
/// Obtained by calling [`PinList::with_iter_pin_mut()`].
pub struct IterPinMut<'a, T> {
    iter: cordyceps::list::IterMut<'a, NodeHeader<T>>,
}

/// An [`Iterator`] over `&mut T` nodes of a [`PinList`]
///
/// Requires `T: Unpin`.
///
/// Obtained by calling [`PinList::with_iter_mut()`].
pub struct IterMut<'a, T: Unpin> {
    iter: cordyceps::list::IterMut<'a, NodeHeader<T>>,
}

/// The inner core of [`PinList`] which is only accessible with the
/// mutex locked.
pub(crate) struct PinListInner<T> {
    pub(crate) list: List<NodeHeader<T>>,
}

// ---- impl PinList ----

impl<R: ScopedRawMutex, T> PinList<R, T> {
    /// Call the given closure with an [`Iter`] which iterates over `&T`s
    ///
    /// The blocking mutex is locked for the duration of the call to `f()`.
    pub fn with_iter<U, F>(&self, f: F) -> U
    where
        F: for<'a> FnOnce(Iter<'a, T>) -> U,
    {
        self.inner.with_lock(|inner| {
            f(Iter {
                iter: inner.list.iter(),
            })
        })
    }

    /// Call the given closure with an [`IterPinMut`] which iterates over `Pin<&mut T>`s
    ///
    /// The blocking mutex is locked for the duration of the call to `f()`.
    ///
    /// If your type implements [`Unpin`], consider using [`PinList::with_iter_mut()`]
    /// if you would prefer an iterator of `&mut T`.
    pub fn with_iter_pin_mut<U, F>(&self, f: F) -> U
    where
        F: for<'a> FnOnce(IterPinMut<'a, T>) -> U,
    {
        self.inner.with_lock(|inner| {
            f(IterPinMut {
                iter: inner.list.iter_mut(),
            })
        })
    }
}

impl<R: ScopedRawMutex, T: Unpin> PinList<R, T> {
    /// Call the given closure with an [`Iter`] which iterates over `Pin<&mut T>`s
    ///
    /// The blocking mutex is locked for the duration of the call to `f()`.
    ///
    /// If your type does NOT implement [`Unpin`], consider using
    /// [`PinList::with_iter_pin_mut()`] which provides an iterator of `Pin<&mut T>`.
    pub fn with_iter_mut<U, F>(&self, f: F) -> U
    where
        F: for<'a> FnOnce(IterMut<'a, T>) -> U,
    {
        self.inner.with_lock(|inner| {
            f(IterMut {
                iter: inner.list.iter_mut(),
            })
        })
    }
}

impl<R: ScopedRawMutex + ConstInit, T> PinList<R, T> {
    /// Create a new [`PinList`].
    ///
    /// Requires that the mutex implements the [`ConstInit`] trait.
    pub const fn new() -> Self {
        Self {
            inner: BlockingMutex::new(PinListInner { list: List::new() }),
        }
    }
}

impl<R: ScopedRawMutex, T> PinList<R, T> {
    /// Create a new [`PinList`] with a given [`ScopedRawMutex`].
    ///
    /// Mainly useful when your mutex cannot be created in const context.
    pub const fn new_manual(r: R) -> Self {
        Self {
            inner: BlockingMutex::const_new(r, PinListInner { list: List::new() }),
        }
    }
}

impl<R: ScopedRawMutex + ConstInit, T> Default for PinList<R, T> {
    fn default() -> Self {
        Self::new()
    }
}

// SAFETY: Access is mediated through a mutex which prevents aliasing access
// If the item is Send, it is safe to implement Send for PinList.
//
// This probably isn't useful, because nodes borrow the PinList when created,
// which means you won't be able to move the PinList, but afaik this is
// technically correct, so we might as well implement it.
unsafe impl<R: ScopedRawMutex, T: Send> Send for PinList<R, T> {}

// SAFETY: Access is mediated through a mutex which prevents aliasing access
// If the item is Send, it is safe to implement Sync for PinList
unsafe impl<R: ScopedRawMutex, T: Send> Sync for PinList<R, T> {}

// ---- impl Iter ----

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|ptr| &ptr.t)
    }
}

// ---- impl IterMut ----

impl<'a, T: Unpin> Iterator for IterMut<'a, T> {
    type Item = &'a mut T;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|ptr| {
            let this = ptr.project();
            let this: Pin<&mut T> = this.t;
            Pin::<&mut T>::into_inner(this)
        })
    }
}

// ---- impl IterPinMut ----

impl<'a, T> Iterator for IterPinMut<'a, T> {
    type Item = Pin<&'a mut T>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|ptr| {
            let this = ptr.project();
            let this: Pin<&mut T> = this.t;
            this
        })
    }
}
