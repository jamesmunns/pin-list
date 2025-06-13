use core::pin::Pin;

use cordyceps::List;
use mutex::{BlockingMutex, ConstInit, ScopedRawMutex};

use super::node::Node;

pub struct PinList<R: ScopedRawMutex, T> {
    pub(crate) inner: BlockingMutex<R, PinListInner<T>>,
}

impl<R: ScopedRawMutex + ConstInit, T> PinList<R, T> {
    pub const fn new() -> Self {
        Self {
            inner: BlockingMutex::new(PinListInner { list: List::new() }),
        }
    }
}

impl<R: ScopedRawMutex, T> PinList<R, T> {
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

unsafe impl<R: ScopedRawMutex, T: Send> Sync for PinList<R, T> {}

pub(crate) struct PinListInner<T> {
    pub(crate) list: List<Node<T>>,
}

impl<R: ScopedRawMutex, T> PinList<R, T> {
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

pub struct Iter<'a, T> {
    iter: cordyceps::list::Iter<'a, Node<T>>,
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|ptr| &ptr.t)
    }
}

pub struct IterMut<'a, T> {
    iter: cordyceps::list::IterMut<'a, Node<T>>,
}

impl<'a, T> Iterator for IterMut<'a, T> {
    type Item = Pin<&'a mut T>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|ptr| {
            let this = ptr.project();
            let this: Pin<&mut T> = this.t;
            this
        })
    }
}
