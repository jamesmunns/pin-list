use core::{marker::PhantomData, pin::Pin, ptr::{addr_of, addr_of_mut, NonNull}};

use cordyceps::{Linked, list::Links};
use mutex::ScopedRawMutex;
use pin_project::pin_project;

use super::list::PinList;

#[pin_project]
pub struct Node<T> {
    pub(crate) links: Links<Node<T>>,
    #[pin]
    pub(crate) t: T,
}

pub struct NodeHandle<'list, 'node, R: ScopedRawMutex, T> {
    list: &'list PinList<R, T>,
    this: NonNull<Node<T>>,
    _this: PhantomData<&'node mut T>,
}

impl<T> Node<T> {
    pub const fn new(t: T) -> Self {
        Self {
            links: Links::new(),
            t,
        }
    }

    pub fn attach<'list, 'node, R: ScopedRawMutex>(
        self: Pin<&'node mut Self>,
        list: &'list PinList<R, T>,
    ) -> NodeHandle<'list, 'node, R, T> {
        let ptr_self: NonNull<Node<T>> = NonNull::from(unsafe {
            self.get_unchecked_mut()
        });
        list.inner.with_lock(|inner| {
            inner.list.push_front(ptr_self);
        });
        NodeHandle {
            this: ptr_self,
            list,
            _this: PhantomData,
        }
    }
}

unsafe impl<T> Linked<Links<Node<T>>> for Node<T> {
    type Handle = NonNull<Node<T>>;

    fn into_ptr(r: Self::Handle) -> core::ptr::NonNull<Self> {
        r
    }

    unsafe fn from_ptr(ptr: core::ptr::NonNull<Self>) -> Self::Handle {
        ptr
    }

    unsafe fn links(target: NonNull<Self>) -> NonNull<Links<Node<T>>> {
        // Safety: using `ptr::addr_of!` avoids creating a temporary
        // reference, which stacked borrows dislikes.
        let node = unsafe { core::ptr::addr_of_mut!((*target.as_ptr()).links) };
        unsafe { NonNull::new_unchecked(node) }
    }
}

impl<R: ScopedRawMutex, T> Drop for NodeHandle<'_, '_, R, T> {
    fn drop(&mut self) {
        self.list.inner.with_lock(|inner| unsafe {
            inner.list.remove(self.this);
        })
    }
}

impl<'list, R: ScopedRawMutex, T> NodeHandle<'list, '_, R, T> {
    pub fn with_lock<U, F: FnOnce(&T) -> U>(&self, f: F) -> U {
        self.list.inner.with_lock(|_inner| {
            let this: &T = unsafe {
                let nt: NonNull<Node<T>> = self.this;
                let t: *const T = addr_of!((*nt.as_ptr()).t);
                &*t
            };

            f(this)
        })
    }

    pub fn with_lock_mut<U, F: FnOnce(Pin<&mut T>) -> U>(&self, f: F) -> U {
        self.list.inner.with_lock(|_inner| {
            let this: Pin<&mut T> = unsafe {
                let nt: NonNull<Node<T>> = self.this;
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
