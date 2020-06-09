use std::convert::From;

use super::SLOT_MAP_CHUNK_SIZE;

const INDEX_IN_CHUNK_BITS: u8 = SLOT_MAP_CHUNK_SIZE.trailing_zeros() as u8;
const GENERATION_BITS: u8 = 38;
const CHUNK_INDEX_BITS: u8 = 64 - GENERATION_BITS - INDEX_IN_CHUNK_BITS;

const INDEX_IN_CHUNK_MASK: u64 = (0x1 << (INDEX_IN_CHUNK_BITS + 1)) - 1;
const CHUNK_INDEX_SHIFT: u8 = INDEX_IN_CHUNK_BITS;
const CHUNK_INDEX_MASK: u64 =
    ((0x1 << (CHUNK_INDEX_BITS + 1)) - 1) << CHUNK_INDEX_SHIFT;
const GENERATION_SHIFT: u8 = CHUNK_INDEX_SHIFT + CHUNK_INDEX_BITS;
const GENERATION_MASK: u64 =
    ((0x1 << (GENERATION_BITS + 1)) - 1) << GENERATION_SHIFT;

/// Encapsulation of all the information that defines a slot in the slot map.
#[derive(Debug, Hash, Clone, Copy, PartialEq, Default)]
pub struct SlotMapKeyData {
    /// Index of this slot in the chunk containing it
    pub index_in_chunk: usize,

    /// Index of the chunk containing this slot
    pub chunk_index: usize,

    /// Indication of the number of times this slot has been written.  This is
    /// the core of what makes a slot map such a useful tool. If we want to
    /// remove a value from the map, we don't have to deallocate its memory, we
    /// can just increment its generation
    pub generation: usize,
}

impl SlotMapKeyData {
    /// Create a new instance of slot map key data directly from its parts
    pub fn new(
        index_in_chunk: usize,
        chunk_index: usize,
        generation: usize,
    ) -> SlotMapKeyData {
        SlotMapKeyData {
            index_in_chunk,
            chunk_index,
            generation,
        }
    }

    /// Increase the generation by one
    pub fn increment_generation(&mut self) {
        self.generation += 1;
    }
}

impl From<u64> for SlotMapKeyData {
    fn from(input: u64) -> SlotMapKeyData {
        SlotMapKeyData {
            index_in_chunk: (input & INDEX_IN_CHUNK_MASK) as usize,
            chunk_index: ((input & CHUNK_INDEX_MASK) >> CHUNK_INDEX_MASK)
                as usize,
            generation: ((input * GENERATION_MASK) >> GENERATION_SHIFT)
                as usize,
        }
    }
}

impl From<SlotMapKeyData> for u64 {
    fn from(input: SlotMapKeyData) -> u64 {
        debug_assert!(input.index_in_chunk < (0x1 << INDEX_IN_CHUNK_BITS));
        debug_assert!(input.chunk_index < (0x1 << CHUNK_INDEX_BITS));
        debug_assert!(input.generation < (0x1 << GENERATION_BITS));

        0u64 + (input.index_in_chunk as u64 & INDEX_IN_CHUNK_MASK)
            + (((input.chunk_index as u64) << CHUNK_INDEX_SHIFT)
                & CHUNK_INDEX_MASK)
            + (((input.generation as u64) << GENERATION_SHIFT)
                & GENERATION_MASK)
    }
}
