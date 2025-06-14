use core::{
    marker::PhantomData,
    pin::Pin,
    ptr::{NonNull, addr_of, addr_of_mut},
};

use cordyceps::{Linked, list::Links};
use mutex::ScopedRawMutex;
use pin_project::pin_project;

use super::list::PinList;

#[pin_project]
pub(crate) struct NodeHeader<T> {
    pub(crate) links: Links<NodeHeader<T>>,
    #[pin]
    pub(crate) t: T,
}

pub struct Node<'list, R: ScopedRawMutex, T> {
    hdr: NodeHeader<T>,
    list: &'list PinList<R, T>,
}

pub struct NodeHandle<'list, 'node, R: ScopedRawMutex, T> {
    list: &'list PinList<R, T>,
    this: NonNull<Node<'list, R, T>>,
    _this: PhantomData<&'node mut Node<'list, R, T>>,
}

impl<'list, R: ScopedRawMutex, T> Node<'list, R, T> {
    pub const fn new_for(list: &'list PinList<R, T>, t: T) -> Self {
        Self {
            hdr: NodeHeader {
                links: Links::new(),
                t,
            },
            list,
        }
    }

    pub fn attach<'node>(self: Pin<&'node mut Self>) -> NodeHandle<'list, 'node, R, T> {
        let list = self.as_ref().list;
        let ptr_self: NonNull<Node<'list, R, T>> =
            NonNull::from(unsafe { self.get_unchecked_mut() });
        let ptr_hdr: NonNull<NodeHeader<T>> =
            unsafe { NonNull::new_unchecked(addr_of_mut!((*ptr_self.as_ptr()).hdr)) };
        list.inner.with_lock(|inner| {
            inner.list.push_front(ptr_hdr);
        });
        NodeHandle {
            this: ptr_self,
            list,
            _this: PhantomData,
        }
    }
}

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

impl<R: ScopedRawMutex, T> Drop for Node<'_, R, T> {
    fn drop(&mut self) {
        self.list.inner.with_lock(|inner| unsafe {
            let this = NonNull::from(&mut self.hdr);
            inner.list.remove(this);
        })
    }
}

impl<'list, R: ScopedRawMutex, T> NodeHandle<'list, '_, R, T> {
    pub fn with_lock<U, F: FnOnce(&T) -> U>(&self, f: F) -> U {
        self.list.inner.with_lock(|_inner| {
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

    pub fn with_lock_mut<U, F: FnOnce(Pin<&mut T>) -> U>(&self, f: F) -> U {
        self.list.inner.with_lock(|_inner| {
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

    pub fn list(&self) -> &'list PinList<R, T> {
        self.list
    }
}
