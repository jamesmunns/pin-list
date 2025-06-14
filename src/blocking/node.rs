//! The Node of a PinList

use core::{
    marker::PhantomData,
    pin::Pin,
    ptr::{NonNull, addr_of, addr_of_mut},
};

use cordyceps::{Linked, list::Links};
use mutex::ScopedRawMutex;
use pin_project::pin_project;

use super::list::PinList;

/// A Node that can be added to a [`PinList`].
///
/// Can be attached to a [`PinList`] by calling [`Node::attach()`] after
/// pinning, which will return a [`NodeHandle`].
///
/// Pinning the node is essential to ensure that the destructor cannot be
/// skipped, as the node is unlinked when `Drop` is called, taking the
/// mutex for a short time to remove the node.
///
/// [`PinList`]: crate::blocking::PinList
///
/// ## Example
///
/// ```rust
/// # // only works with `_docs` active so we have the CS impl
/// # #[cfg(feature = "_docs")]
/// # fn example() {
/// use core::pin::pin;
/// use pinlist::blocking::{PinList, Node};
/// use mutex::raw_impls::cs::CriticalSectionRawMutex as CsRm;
///
/// // Our LIST contains all attached nodes
/// static LIST: PinList<CsRm, u64> = PinList::new();
///
/// // Create the node for the list
/// let node_a = Node::new_for(&LIST, 123);
/// let node_a = pin!(node_a);
/// let hdl_a = node_a.attach();
///
/// // Access through the handle
/// assert_eq!(123, hdl_a.with_lock(|n| *n));
/// // Access through the list
/// LIST.with_iter(|mut i| assert_eq!(123, *i.next().unwrap()))
/// # }
/// # #[cfg(feature = "_docs")]
/// # example()
/// ```
#[must_use = "Nodes must be `attach()`ed to be added to the list"]
pub struct Node<'list, R: ScopedRawMutex, T> {
    hdr: NodeHeader<T>,
    list: &'list PinList<R, T>,
}

/// A handle that represents the [`Node`]s presence in a [`PinList`].
///
/// Dropping the handle does NOT remove the node from the list.
pub struct NodeHandle<'list, 'node, R: ScopedRawMutex, T> {
    list: &'list PinList<R, T>,
    this: NonNull<Node<'list, R, T>>,
    _this: PhantomData<&'node mut Node<'list, R, T>>,
}

/// The portions of the Node that are NOT generic over the lifetime or Mutex
///
/// This is the actual item that appears within the cordyceps linked list, to
/// avoid impossible lifetimes in the PinList itself.
///
/// This header allows for structural pinning of the `T` it contains.
#[pin_project]
pub(crate) struct NodeHeader<T> {
    pub(crate) links: Links<NodeHeader<T>>,
    #[pin]
    pub(crate) t: T,
}

impl<'list, R: ScopedRawMutex, T> Node<'list, R, T> {
    /// Create a new [`Node`] for the given [`PinList`](crate::blocking::PinList).
    pub const fn new_for(list: &'list PinList<R, T>, t: T) -> Self {
        Self {
            hdr: NodeHeader {
                links: Links::new(),
                t,
            },
            list,
        }
    }

    /// Attach the given node to the list it was created with.
    ///
    /// This will return a [`NodeHandle`]. The item will remain in the list
    /// until the `Node` is dropped.
    ///
    /// The mutex will be locked briefly to insert the node in the list.
    pub fn attach<'node>(self: Pin<&'node mut Self>) -> NodeHandle<'list, 'node, R, T> {
        let list = self.as_ref().list;
        // Safety: We consume the Pin'd version of self, to convert it to a NonNull. We will
        // only ever use this as a pinned item, unless T: Unpin.
        let ptr_self: NonNull<Node<'list, R, T>> =
            NonNull::from(unsafe { self.get_unchecked_mut() });

        // Safety: We know self is a valid pointer, so creating a nonnull of a field is
        // also always valid.
        let ptr_hdr: NonNull<NodeHeader<T>> =
            unsafe { NonNull::new_unchecked(addr_of_mut!((*ptr_self.as_ptr()).hdr)) };
        list.inner.with_lock(|inner| {
            inner.list.push_back(ptr_hdr);
        });
        NodeHandle {
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
unsafe impl<T> Linked<Links<NodeHeader<T>>> for NodeHeader<T> {
    type Handle = NonNull<NodeHeader<T>>;

    fn into_ptr(r: Self::Handle) -> core::ptr::NonNull<Self> {
        r
    }

    unsafe fn from_ptr(ptr: core::ptr::NonNull<Self>) -> Self::Handle {
        ptr
    }

    unsafe fn links(target: NonNull<Self>) -> NonNull<Links<NodeHeader<T>>> {
        // Safety: using `ptr::addr_of!` avoids creating a temporary
        // reference, which stacked borrows dislikes.
        let node = unsafe { core::ptr::addr_of_mut!((*target.as_ptr()).links) };
        unsafe { NonNull::new_unchecked(node) }
    }
}

/// Drop the node, unlinking it from the list in the process.
impl<R: ScopedRawMutex, T> Drop for Node<'_, R, T> {
    fn drop(&mut self) {
        // SAFETY: We have the mutex held, meaning we can detach ourselves
        // from the list.
        self.list.inner.with_lock(|inner| unsafe {
            let this = NonNull::from(&mut self.hdr);
            inner.list.remove(this);
        })
    }
}

impl<'list, R: ScopedRawMutex, T> NodeHandle<'list, '_, R, T> {
    /// Access the immutably item within a closure.
    ///
    /// The mutex is locked for the duration of the closure.
    pub fn with_lock<U, F: FnOnce(&T) -> U>(&self, f: F) -> U {
        self.list.inner.with_lock(|_inner| {
            // SAFETY: We hold the lock, and we are providing a &T reference, preventing
            // the item from being moved out
            let this: &T = unsafe {
                let nt: NonNull<Node<'list, R, T>> = self.this;
                let nt: NonNull<NodeHeader<T>> =
                    NonNull::new_unchecked(addr_of_mut!((*nt.as_ptr()).hdr));
                let t: *const T = addr_of!((*nt.as_ptr()).t);
                &*t
            };

            f(this)
        })
    }

    /// Access the item via a pinned mut reference within a closure.
    ///
    /// If your item implements `T: Unpin`, consider using [`NodeHandle::with_lock_mut()`]
    /// to get an `&mut T` directly.
    ///
    /// The mutex is locked for the duration of the closure.
    pub fn with_lock_pin_mut<U, F: FnOnce(Pin<&mut T>) -> U>(&self, f: F) -> U {
        self.list.inner.with_lock(|_inner| {
            // SAFETY: We hold the lock, and we are providing a Pin<&mut T> reference, preventing
            // the item from being moved out
            let this: Pin<&mut T> = unsafe {
                let nt: NonNull<Node<'list, R, T>> = self.this;
                let nt: NonNull<NodeHeader<T>> =
                    NonNull::new_unchecked(addr_of_mut!((*nt.as_ptr()).hdr));
                let t: *mut T = addr_of_mut!((*nt.as_ptr()).t);
                Pin::new_unchecked(&mut *t)
            };

            f(this)
        })
    }

    /// Access the list this Node was created with
    pub fn list(&self) -> &'list PinList<R, T> {
        self.list
    }
}

impl<'list, R: ScopedRawMutex, T: Unpin> NodeHandle<'list, '_, R, T> {
    /// Access the item via a mut reference within a closure.
    ///
    /// The item must implement `T: Unpin`. Consider using [`NodeHandle::with_lock_pin_mut()`]
    /// if your item does not implement `Unpin`.
    ///
    /// The mutex is locked for the duration of the closure.
    pub fn with_lock_mut<U, F: FnOnce(&mut T) -> U>(&self, f: F) -> U {
        self.list.inner.with_lock(|_inner| {
            // SAFETY: We hold the lock, and T: Unpin, so it is safe to provide
            // a mutable reference for the duration of the closure
            let this: &mut T = unsafe {
                let nt: NonNull<Node<'list, R, T>> = self.this;
                let nt: NonNull<NodeHeader<T>> =
                    NonNull::new_unchecked(addr_of_mut!((*nt.as_ptr()).hdr));
                let t: *mut T = addr_of_mut!((*nt.as_ptr()).t);
                &mut *t
            };

            f(this)
        })
    }
}
