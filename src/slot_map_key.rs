use super::SlotMapKeyData;
use std::borrow::Borrow;
use std::convert::From;

/// Trait required for any type used as a slot map key.
pub trait SlotMapKey<T>:
    'static + From<(T, SlotMapKeyData)> + Borrow<SlotMapKeyData>
{
}
