//! # Blocking `PinList`
//!
//! This version of PinList uses a [`BlockingMutex`] to mediate access to nodes.
//!
//! This allows for [pinned] [`Node<T>`]s to be attached to the list, and to allow
//! access to nodes of this list through a shared `&PinList`.
//!
//! Nodes are automatically removed when dropped, and nodes must be pinned when
//! added to the [`PinList`], meaning they cannot be safely forgotten.
//!
//! [`BlockingMutex`]: mutex::BlockingMutex
//! [pinned]: https://doc.rust-lang.org/stable/std/pin/index.html
//!
//! ## Examples
//!
//! A [`PinList`] may be created either as a static, or as an item that can be moved
//! around as part of another structure. As a static, it would look like this:
//!
//! ```rust
//! # // only works with `_docs` active so we have the CS impl
//! # #[cfg(feature = "_docs")]
//! # fn example() {
//! use core::pin::pin;
//! use pinlist::blocking::{PinList, Node};
//! use mutex::raw_impls::cs::CriticalSectionRawMutex as CsRm;
//!
//! // Our LIST contains all attached nodes
//! static LIST: PinList<CsRm, u64> = PinList::new();
//!
//! // Create the nodes for the list
//! let node_a = Node::new_for(&LIST, 123);
//! let node_b = Node::new_for(&LIST, 456);
//!
//! // Pin the nodes, and attach them to the list
//! let node_a = pin!(node_a);
//! let node_b = pin!(node_b);
//! let hdl_a = node_a.attach();
//! let hdl_b = node_b.attach();
//!
//! // Access can be made through the handle, while holding the lock
//! assert_eq!(123, hdl_a.with_lock(|a| *a));
//! assert_eq!(456, hdl_b.with_lock(|b| *b));
//!
//! // We can access all items through the list
//! let items = LIST.with_iter(|n| n.copied().collect::<Vec<_>>());
//! assert_eq!(&[123, 456], items.as_slice());
//!
//! // We can create an ephemeral item...
//! {
//!     let node_c = Node::new_for(&LIST, 789);
//!     let node_c = pin!(node_c);
//!
//!     // Node C is not added until we attach
//!     let items = LIST.with_iter(|n| n.copied().collect::<Vec<_>>());
//!     assert_eq!(&[123, 456], items.as_slice());
//!
//!     let _hdl_c = node_c.attach();
//!     let items = LIST.with_iter(|n| n.copied().collect::<Vec<_>>());
//!     assert_eq!(&[123, 456, 789], items.as_slice());
//!     // node_c is dropped here
//! }
//!
//! // We can see node_c has been removed
//! let items = LIST.with_iter(|n| n.copied().collect::<Vec<_>>());
//! assert_eq!(&[123, 456], items.as_slice());
//! # }
//! # #[cfg(feature = "_docs")]
//! # example();
//! ```

mod list;
mod node;
pub mod dynlist;

pub use list::{Iter, IterMut, IterPinMut, PinList};
pub use node::{Node, NodeHandle};
