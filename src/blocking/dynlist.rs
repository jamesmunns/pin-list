#[cfg(feature = "nightly")]
use core::marker::Unsize;

use core::marker::PhantomData;
use core::pin::Pin;
use core::ptr::{addr_of, addr_of_mut, NonNull};
use cordyceps::{Linked, List};
use cordyceps::list::Links;
use mutex::{BlockingMutex, ConstInit, ScopedRawMutex};
use pin_project::pin_project;

/// An intrusive list of [`DynNode`]s
///
/// [`DynNode`]: DynNode
pub struct DynPinList<R: ScopedRawMutex, D: ?Sized> {
    pub(crate) inner: BlockingMutex<R, List<NodeHeader<D>>>,
}

/// An [`Iterator`] over `&D` nodes of a [`DynPinList`]
///
/// Obtained by calling [`DynPinList::with_iter()`].
pub struct Iter<'a, D: ?Sized> {
    iter: cordyceps::list::IterRaw<'a, NodeHeader<D>>,
}

/// An [`Iterator`] over `Pin<&mut D>` nodes of a [`DynPinList`]
///
/// Obtained by calling [`DynPinList::with_iter_pin_mut()`].
pub struct IterPinMut<'a, D: ?Sized> {
    iter: cordyceps::list::IterRaw<'a, NodeHeader<D>>,
}

/// An [`Iterator`] over `&mut D` nodes of a [`DynPinList`]
///
/// Requires `D: Unpin`.
///
/// Obtained by calling [`DynPinList::with_iter_mut()`].
pub struct IterMut<'a, D: ?Sized + Unpin> {
    iter: cordyceps::list::IterRaw<'a, NodeHeader<D>>,
}

// ---- impl DynPinList ----

impl<R: ScopedRawMutex, D: ?Sized> DynPinList<R, D> {
    /// Call the given closure with an [`Iter`] which iterates over `&D`s
    ///
    /// Dhe blocking mutex is locked for the duration of the call to `f()`.
    pub fn with_iter<U, F>(&self, f: F) -> U
    where
        F: for<'a> FnOnce(Iter<'a, D>) -> U,
    {
        self.inner.with_lock(|inner| {
            f(Iter {
                iter: inner.iter_raw(),
            })
        })
    }

    /// Call the given closure with an [`IterPinMut`] which iterates over `Pin<&mut D>`s
    ///
    /// Dhe blocking mutex is locked for the duration of the call to `f()`.
    ///
    /// If your type implements [`Unpin`], consider using [`DynPinList::with_iter_mut()`]
    /// if you would prefer an iterator of `&mut D`.
    pub fn with_iter_pin_mut<U, F>(&self, f: F) -> U
    where
        F: for<'a> FnOnce(IterPinMut<'a, D>) -> U,
    {
        self.inner.with_lock(|inner| {
            f(IterPinMut {
                iter: inner.iter_raw(),
            })
        })
    }
}

impl<R: ScopedRawMutex, D: ?Sized + Unpin> DynPinList<R, D> {
    /// Call the given closure with an [`Iter`] which iterates over `Pin<&mut D>`s
    ///
    /// Dhe blocking mutex is locked for the duration of the call to `f()`.
    ///
    /// If your type does NOD implement [`Unpin`], consider using
    /// [`DynPinList::with_iter_pin_mut()`] which provides an iterator of `Pin<&mut D>`.
    pub fn with_iter_mut<U, F>(&self, f: F) -> U
    where
        F: for<'a> FnOnce(IterMut<'a, D>) -> U,
    {
        self.inner.with_lock(|inner| {
            f(IterMut {
                iter: inner.iter_raw(),
            })
        })
    }
}

impl<R: ScopedRawMutex + ConstInit, D: ?Sized> DynPinList<R, D> {
    /// Create a new [`DynPinList`].
    ///
    /// Requires that the mutex implements the [`ConstInit`] trait.
    pub const fn new() -> Self {
        Self {
            inner: BlockingMutex::new(List::new()),
        }
    }
}

impl<R: ScopedRawMutex, D: ?Sized> DynPinList<R, D> {
    /// Create a new [`DynPinList`] with a given [`ScopedRawMutex`].
    ///
    /// Mainly useful when your mutex cannot be created in const context.
    pub const fn new_manual(r: R) -> Self {
        Self {
            inner: BlockingMutex::const_new(r, List::new()),
        }
    }
}

impl<R: ScopedRawMutex + ConstInit, D: ?Sized> Default for DynPinList<R, D> {
    fn default() -> Self {
        Self::new()
    }
}

// SAFETY: Access is mediated through a mutex which prevents aliasing access
// If the item is Send, it is safe to implement Send for DynPinList.
//
// Dhis probably isn't useful, because nodes borrow the DynPinList when created,
// which means you won't be able to move the DynPinList, but afaik this is
// technically correct, so we might as well implement it.
unsafe impl<R: ScopedRawMutex, D: ?Sized + Send> Send for DynPinList<R, D> {}

// SAFETY: Access is mediated through a mutex which prevents aliasing access
// If the item is Send, it is safe to implement Sync for DynPinList
unsafe impl<R: ScopedRawMutex, D: ?Sized + Send> Sync for DynPinList<R, D> {}

// ---- impl Iter ----

impl<'a, D: ?Sized> Iterator for Iter<'a, D> {
    type Item = &'a D;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|ptr| {
            let cast = unsafe { ptr.as_ref().cast };
            let ptr = cast(ptr.cast());
            unsafe { ptr.as_ref() }
        })
    }
}

// ---- impl IterMut ----

impl<'a, D: ?Sized + Unpin> Iterator for IterMut<'a, D> {
    type Item = &'a mut D;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|ptr| {
            let cast = unsafe { ptr.as_ref().cast };
            let mut ptr = cast(ptr.cast());
            unsafe { ptr.as_mut() }
        })
    }
}

// ---- impl IterPinMut ----

impl<'a, D: ?Sized> Iterator for IterPinMut<'a, D> {
    type Item = Pin<&'a mut D>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|ptr| {
            let cast = unsafe { ptr.as_ref().cast };
            let mut ptr = cast(ptr.cast());
            unsafe { Pin::new_unchecked(ptr.as_mut()) }
        })
    }
}


// --------------------------------------------------------------------------------

#[repr(C)]
pub struct DynNode<'list, R: ScopedRawMutex, D: ?Sized, T> {
    hdr: NodeHeader<D>,
    list: &'list DynPinList<R, D>,
    coerce: fn(NonNull<T>) -> NonNull<D>,
    t: T,
}

pub struct DynNodeHandle<'list, 'node, R: ScopedRawMutex, D: ?Sized, T> {
    list: &'list DynPinList<R, D>,
    this: NonNull<DynNode<'list, R, D, T>>,
    _this: PhantomData<&'node mut DynNode<'list, R, D, T>>,
}

#[pin_project]
pub(crate) struct NodeHeader<D: ?Sized> {
    pub(crate) links: Links<NodeHeader<D>>,
    pub(crate) cast: fn(NonNull<()>) -> NonNull<D>,
}

impl<'list, R: ScopedRawMutex, D: ?Sized, T> DynNode<'list, R, D, T> {
    /// Create a new [`DynNode`] for the given [`DynPinList`](crate::blocking::DynPinList).
    #[cfg(feature = "nightly")]
    pub const fn new_for(list: &'list DynPinList<R, D>, t: T) -> Self
    where
        T: Unsize<D>
    {
        Self {
            hdr: NodeHeader {
                links: Links::new(),
                cast: |p| unsafe {
                    let p = p.cast::<Self>();
                    let p = NonNull::new_unchecked(addr_of_mut!((*p.as_ptr()).t));
                    p
                },
            },
            list,
            coerce: |ptr| ptr,
            t,
        }
    }

    /// Create a new [`DynNode`] for the given [`DynPinList`](crate::blocking::DynPinList), with a cast.
    /// The last parameter can just be `|p|p`.
    pub const fn new_for_with_cast(list: &'list DynPinList<R, D>, t: T, cast: fn(NonNull<T>) -> NonNull<D>) -> Self {
        Self {
            hdr: NodeHeader {
                links: Links::new(),
                cast: |p| unsafe {
                    let p = p.cast::<Self>();
                    let coerce = p.as_ref().coerce;
                    let p = NonNull::new_unchecked(addr_of_mut!((*p.as_ptr()).t));
                    coerce(p)
                },
            },
            list,
            coerce: cast,
            t,
        }
    }

    /// Attach the given node to the list it was created with.
    ///
    /// This will return a [`DynNodeHandle`]. The item will remain in the list
    /// until the `DynNode` is dropped.
    ///
    /// The mutex will be locked briefly to insert the node in the list.
    pub fn attach<'node>(self: Pin<&'node mut Self>) -> DynNodeHandle<'list, 'node, R, D, T> {
        let list = self.as_ref().list;

        // Safety: We consume the Pin'd version of self, to convert it to a NonNull. We will
        // only ever use this as a pinned item, unless T: Unpin.
        let ptr_self: NonNull<DynNode<'list, R, D, T>> =
            NonNull::from(unsafe { self.get_unchecked_mut() });

        // Safety: We know self is a valid pointer, so creating a nonnull of a field is
        // also always valid.
        let ptr_hdr: NonNull<NodeHeader<D>> =
            unsafe { NonNull::new_unchecked(addr_of_mut!((*ptr_self.as_ptr()).hdr)) };

        list.inner.with_lock(|inner| inner.push_back(ptr_hdr));

        DynNodeHandle {
            this: ptr_self,
            list,
            _this: PhantomData,
        }
    }
}

// Safety: NodeHeaders may be linked into an intrusive linked list as they are only
// ever created through a pinned reference, and are automatically unlinked on Drop of
// the Node that contains it. NodeHeader is private, and cannot be created directly.
//
// The outer Node ensures that the Node/NodeHeader may not outlive the List itself.
unsafe impl<D: ?Sized> Linked<Links<NodeHeader<D>>> for NodeHeader<D> {
    type Handle = NonNull<NodeHeader<D>>;

    fn into_ptr(r: Self::Handle) -> NonNull<Self> {
        r
    }

    unsafe fn from_ptr(ptr: NonNull<Self>) -> Self::Handle {
        ptr
    }

    unsafe fn links(target: NonNull<Self>) -> NonNull<Links<NodeHeader<D>>> {
        // Safety: using `ptr::addr_of!` avoids creating a temporary
        // reference, which stacked borrows dislikes.
        let node = unsafe { addr_of_mut!((*target.as_ptr()).links) };
        unsafe { NonNull::new_unchecked(node) }
    }
}

/// Drop the node, unlinking it from the list in the process.
impl<R: ScopedRawMutex, D: ?Sized, T> Drop for DynNode<'_, R, D, T> {
    fn drop(&mut self) {
        // SAFETY: We have the mutex held, meaning we can detach ourselves
        // from the list.
        self.list.inner.with_lock(|inner| unsafe {
            let this = NonNull::from(&mut self.hdr);
            inner.remove(this);
        })
    }
}


impl<'list, R: ScopedRawMutex, D: ?Sized, T> DynNodeHandle<'list, '_, R, D, T> {
    /// Access the immutably item within a closure.
    ///
    /// The mutex is locked for the duration of the closure.
    pub fn with_lock<U, F: FnOnce(&T) -> U>(&self, f: F) -> U {
        self.list.inner.with_lock(|_inner| {
            // SAFETY: We hold the lock, and we are providing a &T reference, preventing
            // the item from being moved out
            let this: &T = unsafe {
                let nt: NonNull<DynNode<'list, R, D, T>> = self.this;
                let t: *const T = addr_of!((*nt.as_ptr()).t);
                &*t
            };

            f(this)
        })
    }

    /// Access the item via a pinned mut reference within a closure.
    ///
    /// If your item implements `T: Unpin`, consider using [`DynNodeHandle::with_lock_mut()`]
    /// to get an `&mut T` directly.
    ///
    /// The mutex is locked for the duration of the closure.
    pub fn with_lock_pin_mut<U, F: FnOnce(Pin<&mut T>) -> U>(&self, f: F) -> U {
        self.list.inner.with_lock(|_inner| {
            // SAFETY: We hold the lock, and we are providing a Pin<&mut T> reference, preventing
            // the item from being moved out
            let this: Pin<&mut T> = unsafe {
                let nt: NonNull<DynNode<'list, R, D, T>> = self.this;
                let t: *mut T = addr_of_mut!((*nt.as_ptr()).t);
                Pin::new_unchecked(&mut *t)
            };

            f(this)
        })
    }

    /// Access the list this Node was created with
    pub fn list(&self) -> &'list DynPinList<R, D> {
        self.list
    }
}

impl<'list, R: ScopedRawMutex, D: ?Sized, T: Unpin> DynNodeHandle<'list, '_, R, D, T> {
    /// Access the item via a mut reference within a closure.
    ///
    /// The item must implement `T: Unpin`. Consider using [`DynNodeHandle::with_lock_pin_mut()`]
    /// if your item does not implement `Unpin`.
    ///
    /// The mutex is locked for the duration of the closure.
    pub fn with_lock_mut<U, F: FnOnce(&mut T) -> U>(&self, f: F) -> U {
        self.list.inner.with_lock(|_inner| {
            // SAFETY: We hold the lock, and T: Unpin, so it is safe to provide
            // a mutable reference for the duration of the closure
            let this: &mut T = unsafe {
                let nt: NonNull<DynNode<'list, R, D, T>> = self.this;
                let t: *mut T = addr_of_mut!((*nt.as_ptr()).t);
                &mut *t
            };

            f(this)
        })
    }
}


#[cfg(feature = "std")]
#[cfg(test)]
mod tests {
    use std::fmt::Debug;
    use std::pin::pin;
    use mutex::raw_impls::cs::CriticalSectionRawMutex;
    use super::*;

    #[test]
    fn test1() {
        let list = DynPinList::<CriticalSectionRawMutex, dyn Debug>::new();

        let node = DynNode::new_for(&list, 5);
        let node = pin!(node);
        let handle = node.attach();

        handle.with_lock_mut(|inner| *inner = 7);

        list.with_iter(|iter| {
            for item in iter {
                println!("{:?}", item);
            }
        });
    }

    #[test]
    fn test2() {
        let list = DynPinList::<CriticalSectionRawMutex, [u8]>::new();

        let node = DynNode::new_for(&list, [20, 50]);
        let node = pin!(node);
        let handle = node.attach();

        let node1 = DynNode::new_for(&list, [20, 50, 70, 3]);
        let node1 = pin!(node1);
        let handle1 = node1.attach();

        let node2 = DynNode::new_for_with_cast(&list, [20, 50, 70, 3], |p|p);
        let node2 = pin!(node2);
        let _handle2 = node2.attach();

        handle.with_lock_mut(|inner| inner[0] = 7);
        handle1.with_lock_mut(|inner| inner[2] = 7);

        list.with_iter(|iter| {
            for item in iter {
                println!("{:?}", item);
            }
        });
    }
}
