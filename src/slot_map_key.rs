use super::SlotMapKeyData;
use std::convert::From;

pub trait SlotMapKey<T>: 'static + From<(T, SlotMapKeyData)> {
    fn get_slot_map_key_data(&self) -> &SlotMapKeyData;
}
