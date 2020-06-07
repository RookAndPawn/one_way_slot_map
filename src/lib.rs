#[macro_use]
#[cfg(test)]
extern crate static_assertions;

#[macro_export]
macro_rules! define_key_type (
    ($key_type:ident<$pointer_type:ty> $(: $derive_1:ident $(+ $more_derives:ident)* )?) => {

        $(#[derive($derive_1 $(, $more_derives)*)])?
        pub struct $key_type {
            pointer: $pointer_type,
            slot_key: SlotMapKeyData,
        }

        impl SlotMapKey<$pointer_type> for $key_type {
            fn get_slot_map_key_data(&self) -> &SlotMapKeyData {
                &self.slot_key
            }
        }

        impl From<($pointer_type, SlotMapKeyData)> for $key_type {
            fn from(f: ($pointer_type, SlotMapKeyData)) -> Self {
                let (pointer, slot_key) = f;
                $key_type { pointer, slot_key }
            }
        }
    };
);

pub const SLOT_MAP_CHUNK_SIZE: usize = 256;

pub use slot_map::SlotMap;
pub use slot_map_key::SlotMapKey;
pub use slot_map_key_data::SlotMapKeyData;

mod slot_map;
mod slot_map_key;
mod slot_map_key_data;
