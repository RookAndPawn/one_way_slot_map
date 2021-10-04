use std::convert::From;

use super::SLOT_MAP_CHUNK_SIZE;

const INDEX_IN_CHUNK_BITS: u8 = SLOT_MAP_CHUNK_SIZE.trailing_zeros() as u8;
const CHUNK_INDEX_BITS: u8 = 32;
const GENERATION_BITS: u8 = 64 - CHUNK_INDEX_BITS - INDEX_IN_CHUNK_BITS;

const INDEX_IN_CHUNK_MASK: u64 = (0x1 << INDEX_IN_CHUNK_BITS) - 1;
const CHUNK_INDEX_SHIFT: u8 = INDEX_IN_CHUNK_BITS;
const CHUNK_INDEX_MASK: u64 =
    ((0x1 << CHUNK_INDEX_BITS) - 1) << CHUNK_INDEX_SHIFT;
const GENERATION_SHIFT: u8 = CHUNK_INDEX_SHIFT + CHUNK_INDEX_BITS;
const GENERATION_MASK: u64 = ((0x1 << GENERATION_BITS) - 1) << GENERATION_SHIFT;

const MAX_INDEX_IN_CHUNK: u16 = INDEX_IN_CHUNK_MASK as u16;
const MAX_GENERATION: u32 = (0x1 << GENERATION_BITS) - 1;

/// Encapsulation of all the information that defines a slot in the slot map.
#[derive(Debug, Hash, Clone, Copy, PartialEq, Default, Eq)]
pub struct SlotMapKeyData {
    /// Index of this slot in the chunk containing it
    pub(crate) index_in_chunk: u16,

    /// Index of the chunk containing this slot
    pub(crate) chunk_index: u32,

    /// Indication of the number of times this slot has been written.  This is
    /// the core of what makes a slot map such a useful tool. If we want to
    /// remove a value from the map, we don't have to deallocate its memory, we
    /// can just increment its generation
    pub(crate) generation: u32,
}

impl SlotMapKeyData {
    /// Increase the generation by one, and wraps when the generation
    /// passes the max.
    pub(crate) fn increment_generation(&mut self) {
        match &mut self.generation {
            gen if *gen < MAX_GENERATION => *gen += 1,
            gen if *gen == MAX_GENERATION => *gen = 0,
            _ => {
                panic!(
                    "Generation: {} is out of the range allowed by mask: {}",
                    self.generation, MAX_GENERATION
                );
            }
        }
    }

    /// Swap the chunk index and index in chunk fields between self and other
    pub(crate) fn swap_coordinates(&mut self, other: &mut Self) {
        let swap_chunk_index = self.chunk_index;
        self.chunk_index = other.chunk_index;
        other.chunk_index = swap_chunk_index;

        let swap_index_in_chunk = self.index_in_chunk;
        self.index_in_chunk = other.index_in_chunk;
        other.index_in_chunk = swap_index_in_chunk;
    }

    /// Increment the coordinates of this slot map key data. It the index in
    /// chunk wraps (when the maximum index in chunk is reached) increment the
    /// chunk index and return true, and otherwise return false
    pub(crate) fn increment_coordinates(&mut self) -> bool {
        if self.index_in_chunk == MAX_INDEX_IN_CHUNK {
            self.index_in_chunk = 0;
            self.chunk_index += 1;
            true
        } else {
            self.index_in_chunk += 1;
            false
        }
    }

    /// Checks the generation to see if the slot associated with this key data
    /// is filled (even)
    pub(crate) fn is_filled(&self) -> bool {
        self.generation % 2 == 0
    }
}

impl From<u64> for SlotMapKeyData {
    fn from(input: u64) -> SlotMapKeyData {
        SlotMapKeyData {
            index_in_chunk: (input & INDEX_IN_CHUNK_MASK) as u16,
            chunk_index: ((input & CHUNK_INDEX_MASK) >> CHUNK_INDEX_SHIFT)
                as u32,
            generation: ((input & GENERATION_MASK) >> GENERATION_SHIFT) as u32,
        }
    }
}

impl From<SlotMapKeyData> for u64 {
    fn from(input: SlotMapKeyData) -> u64 {
        (input.index_in_chunk as u64 & INDEX_IN_CHUNK_MASK)
            + (((input.chunk_index as u64) << CHUNK_INDEX_SHIFT)
                & CHUNK_INDEX_MASK)
            + (((input.generation as u64) << GENERATION_SHIFT)
                & GENERATION_MASK)
    }
}

#[test]
fn test_coordinate_serialization() {
    let inc: u64 = 91;

    for i in 0..10_000 {
        let v = i * inc;
        let key = SlotMapKeyData::from(v);

        assert_eq!(v, u64::from(key));
    }
}

#[test]
fn test_generation_serialization() {
    let inc: u32 = 91;

    let mut key = SlotMapKeyData::from(1340850812u64);

    for i in 0..10_000 {
        let g = i * inc;
        key.generation = g;

        assert_eq!(key, SlotMapKeyData::from(u64::from(key)));
    }
}
