use super::SlotMapKeyData;
use std::convert::From;

/// Trait required for any type used as a slot map key.
pub trait SlotMapKey<T>: 'static + From<(T, SlotMapKeyData)> {
    /// Get a ref to the slot map key data for this key
    fn get_slot_map_key_data(&self) -> &SlotMapKeyData;
}
