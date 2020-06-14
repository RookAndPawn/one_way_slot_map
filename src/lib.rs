//! An implementation of SlotMap with minimal restrictions on Keys and Values
//!
//! This is an implementation of the slot map data structure similar to
//! [SlotMap](https://github.com/orlp/slotmap) with fewer restrictions and the
//! ability to embed data inside the key objects. The "one-way" moniker for
//! this crate comes from an implementation detail that prevent inserted values
//! from being taken out again unless they are replaced with another instance.
//! Values that are inserted can be referenced, and written to, but ownership of
//! the values remains with the map even after the value is "removed".
//!
//! The data structure uses fixed size chunks (like
//! [SlotMap's DenseSlotMap](https://docs.rs/slotmap/0.4.0/slotmap/dense/struct.DenseSlotMap.html)),
//! so lookups require 2 steps of indirection.
//!
//! # Example Usage:
//! First create a Key Class with an embedded data type
//!
//! ```
//! use one_way_slot_map::*;
//!
//! // Define a simple key with an embedded usize
//! define_key_type!(DemoKey<usize>);
//!
//! // Or define a less-simple key with some derived traits
//! define_key_type!(TestKeyWithDerives<usize> : Copy + Clone + Debug);
//!
//! //Then create a slot map and use the key for crud operations
//! let mut slot_map = SlotMap::new();
//!
//! let key: DemoKey = slot_map.insert(0, "Demo!");
//! assert_eq!(Some(&"Demo!"), slot_map.get(&key));
//! let slot = slot_map.get_mut(&key).unwrap();
//! *slot = "Updated!";
//!
//! assert_eq!(Some(&mut "Updated!"), slot_map.remove(&key));
//! assert_eq!(None, slot_map.get(&key));
//! ```
#![warn(
    missing_docs,
    rust_2018_idioms,
    missing_debug_implementations,
    intra_doc_link_resolution_failure,
    clippy::all
)]

#[macro_use]
#[cfg(test)]
extern crate static_assertions;

/// Macro for creating a simple Key type for one-way slot maps. Key types can be
/// created from scratch, but for most cases, this will produce what you want
#[macro_export]
macro_rules! define_key_type (
    ($key_type:ident<$pointer_type:ty> $(: $derive_1:ident $(+ $more_derives:ident)* )?) => {

        $(#[derive($derive_1 $(, $more_derives)*)])?
        pub struct $key_type {
            pointer: $pointer_type,
            slot_key: one_way_slot_map::SlotMapKeyData,
        }

        impl one_way_slot_map::SlotMapKey<$pointer_type> for $key_type {
            fn get_slot_map_key_data(&self) -> &one_way_slot_map::SlotMapKeyData {
                &self.slot_key
            }
        }

        impl From<($pointer_type, one_way_slot_map::SlotMapKeyData)> for $key_type {
            fn from(f: ($pointer_type, one_way_slot_map::SlotMapKeyData)) -> Self {
                let (pointer, slot_key) = f;
                $key_type { pointer, slot_key }
            }
        }
    };
);

/// This tells the size of the chunks used by the slot map. I'm not sure why
/// or how this would be used, but maybe it's good to know
pub const SLOT_MAP_CHUNK_SIZE: usize = 256;

pub use slot_map::SlotMap;
pub use slot_map_key::SlotMapKey;
pub use slot_map_key_data::SlotMapKeyData;
// pub use slot_map_value_iterator::SlotMapValueIterator;

mod slot_map;
mod slot_map_key;
mod slot_map_key_data;
// mod slot_map_value_iterator;
