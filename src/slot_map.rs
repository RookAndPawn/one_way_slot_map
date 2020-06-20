use super::{SlotMapKey, SlotMapKeyData};
use std::marker::PhantomData;

/// Size of the individual array chunks in the slot map
pub const SLOT_MAP_CHUNK_SIZE: usize = 256;

// Require the chunk size to be a power of 2
#[cfg(test)]
mod sanity_checks {
    const_assert_eq!(super::SLOT_MAP_CHUNK_SIZE.count_ones(), 1u32);
}

/// Encapsulation of the slot storage objects to make the borrow checker happy
struct Slots<T> {
    // This will be replaced with Box<[MaybeUninit] when assumeInitRef is stable
    current_chunk: Box<[Option<(SlotMapKeyData, T)>; SLOT_MAP_CHUNK_SIZE]>,
    filled_chunks: Vec<Box<[(SlotMapKeyData, T); SLOT_MAP_CHUNK_SIZE]>>,
    current_chunk_index: u32,
    current_chunk_cursor: u16,
}

impl<T> Slots<T> {
    pub fn new() -> Slots<T> {
        Slots {
            current_chunk: Box::new(
                array_macro::array![None; SLOT_MAP_CHUNK_SIZE],
            ),
            filled_chunks: Vec::new(),
            current_chunk_index: Default::default(),
            current_chunk_cursor: Default::default(),
        }
    }

    fn get_slot(&self, key: &SlotMapKeyData) -> Option<&(SlotMapKeyData, T)> {
        if key.chunk_index <= self.filled_chunks.len() as u32 {
            self.filled_chunks
                .get(key.chunk_index as usize)
                .unwrap()
                .get(key.index_in_chunk as usize)
        } else {
            self.current_chunk
                .get(key.index_in_chunk as usize)
                .unwrap()
                .as_ref()
        }
    }

    /// Get the slot at the coordinates in the given key.  This method does not
    /// check to ensure that the given key's chunk index is within in the range
    /// of the existing storage vec, but there are also no explicit unwraps here
    /// either, so you get what you get
    fn get_storage_slot_mut(
        &mut self,
        key: &SlotMapKeyData,
    ) -> Option<&mut (SlotMapKeyData, T)> {
        self.filled_chunks
            .get_mut(key.chunk_index as usize)
            .and_then(|chunk| chunk.get_mut(key.index_in_chunk as usize))
    }

    /// Get the slot in the current chunk indicated by the given key. This
    /// method does not check to make sure that the chunk index in the given key
    /// matches the current chunk's index. The index within the chunk is
    /// validated on creation of the key
    fn get_current_chunk_slot_mut(
        &mut self,
        key: &SlotMapKeyData,
    ) -> &mut Option<(SlotMapKeyData, T)> {
        self.current_chunk
            .get_mut(key.index_in_chunk as usize)
            .expect("Invalid index in chunk")
    }

    /// Get a mutable reference to the slot indicated by the coordinates in the
    /// given key. The reason this is get "existing" slot is because it will
    /// return None if a non-initialized slot in the current chunk is requested
    /// rather than a mutable reference to the uninitialized slot
    fn get_existing_slot_mut(
        &mut self,
        key: &SlotMapKeyData,
    ) -> Option<&mut (SlotMapKeyData, T)> {
        if key.chunk_index < self.current_chunk_index {
            self.get_storage_slot_mut(key)
        } else if key.index_in_chunk < self.current_chunk_cursor {
            self.get_current_chunk_slot_mut(key).as_mut()
        } else {
            None
        }
    }
}

/// Implementation of a slot map that limits the restrictions on slotted keys
/// and values by preventing retrieval of original values without explicit
/// replacement
pub struct SlotMap<K, P, T>
where
    K: SlotMapKey<P>,
{
    slots: Slots<T>,
    next_open_slot: SlotMapKeyData,
    len: usize,

    phantom_k: PhantomData<K>,
    phantom_p: PhantomData<P>,
}

// impl<K, P, T> std::fmt::Debug for SlotMap<K, P, T>
// where
//     T: std::fmt::Debug,
//     K: SlotMapKey<P>,
// {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         f.debug_list().entries(self.iter()).finish()
//     }
// }

impl<K, P, T> SlotMap<K, P, T>
where
    K: SlotMapKey<P>,
{
    /// Create a new default simple slot map
    pub fn new() -> SlotMap<K, P, T> {
        SlotMap {
            next_open_slot: Default::default(),
            len: Default::default(),

            phantom_k: PhantomData::default(),
            phantom_p: PhantomData::default(),
        }
    }

    /// Get the number of items in the slot map
    ///
    /// ```
    /// # use one_way_slot_map::*;
    /// # define_key_type!(TestKey<()>);
    /// let mut map = SlotMap::<TestKey,(),usize>::new();
    ///
    /// let _ = map.insert((),10);
    /// let _ = map.insert((),42);
    ///
    /// assert_eq!(2, map.len());
    /// ```
    pub fn len(&self) -> usize {
        self.len
    }

    /// Move the current chunk into storage
    fn move_current_chunk_to_storage(&mut self) {
        let storage_chunk = Box::new(array_macro::array![|i| {
                self.current_chunk
                    .get_mut(i)
                    .expect("Expected correctly sized chunk")
                    .take()
                    .expect("Expected all slots in current chunk to be filled")
            } ; SLOT_MAP_CHUNK_SIZE]);

        self.storage.push(storage_chunk);
        self.current_chunk_index = self.storage.len() as u32;
        self.current_chunk_cursor = 0;
    }

    /// insert the given item into the slot map and return its key
    pub fn insert(&mut self, pointer: P, value: T) -> K {
        let next_slot = &mut self.next_open_slot;

        let key_data = if next_slot.chunk_index < self.current_chunk_index
            || next_slot.index_in_chunk < self.current_chunk_cursor
        {
            let (new_next_slot, old_val) = self
                .get_storage_slot_mut(next_slot)
                .expect("invalid next slot pointer");
            *old_val = value;
            new_next_slot.increment_generation();
            new_next_slot.swap_coordinates(next_slot);
            *new_next_slot
        } else {
            let key_data = *next_slot;
            let slot_opt = self.get_current_chunk_slot_mut(next_slot);
            *slot_opt = Some((*next_slot, value));
            if self.next_open_slot.increment_coordinates() {
                self.move_current_chunk_to_storage()
            }
            key_data
        };

        K::from((pointer, key_data))
    }

    /// Get the value for the given key if there is one
    pub fn get(&self, key: &K) -> Option<&T> {
        let key_data = key.get_slot_map_key_data();

        self.get_slot(key_data)
            .filter(|slot| slot.0.is_filled())
            .filter(|slot| slot.0.generation == key_data.generation)
            .map(|slot| &slot.1)
    }

    /// Get the value for the given key if there is one
    pub fn get_mut(&mut self, key: &K) -> Option<&mut T> {
        let key_data = key.get_slot_map_key_data();

        self.get_existing_slot_mut(key_data)
            .filter(|slot| slot.0.is_filled())
            .filter(|slot| slot.0.generation == key_data.generation)
            .map(|slot| &mut slot.1)
    }
    /// Remove the item at the given index and return a mutable ref to the
    /// item removed if there was one
    pub fn remove(&mut self, key: &K) -> Option<&mut T> {
        let key_data = key.get_slot_map_key_data();

        self.get_existing_slot_mut(key_data)
            .filter(|slot| slot.0.is_filled())
            .filter(|slot| slot.0.generation == key_data.generation)
            .map(|(key, value)| {
                key.increment_generation();
                key.swap_coordinates(&mut self.next_open_slot);
                value
            })
    }
}
/// Clears all the values in the slot map.  This can be a memory intensive
/// operation because we will have to write information for every non-empty
/// slot into the queue of slots that can now be used
// pub fn clear(&mut self) {
//     let current_chunk_index = self.current_chunk_index;

//     // Create an iterator of all the full slots and their generation
//     let full_chunk_key_iter = self.storage.iter_mut().enumerate().flat_map(
//         |(chunk_idx, chunk)| {
//             chunk.iter_mut().enumerate().map(
//                 move |(idx_in_cnk, (gen, _))| (chunk_idx, idx_in_cnk, gen),
//             )
//         },
//     );

//     // Create an iterator of all the slots in the current chunk that have
//     // been written to and their generation
//     let curr_chunk_key_iter = self
//         .current_chunk
//         .iter_mut()
//         .enumerate()
//         .map(|(idx_in_cnk, slot_opt)| {
//             slot_opt
//                 .as_mut()
//                 .map(|(gen, _)| (current_chunk_index, idx_in_cnk, gen))
//         })
//         .filter_map(|v| v);

//     let return_queue = &mut self.queue;

//     // iterate over bother the full and working chunks, and empty the filled
//     // slots (by incrementing their generation) and adding them to the queue
//     // of slots that can be written to
//     full_chunk_key_iter
//         .chain(curr_chunk_key_iter)
//         .filter(|(_, _, gen)| (**gen % 2) == 0)
//         .map(|(chunk_idx, idx_in_chunk, gen)| {
//             *gen += 1;
//             SlotMapKeyData::new(chunk_idx, idx_in_chunk, *gen)
//         })
//         .for_each(|key| return_queue.push_back(key));
// }

// /// Construct an iterator over all the values in the slot map
// pub fn iter<'a>(&'a self) -> impl Iterator<Item = &'a T> {
//     let full_chunks_iter =
//         self.storage.iter().flat_map(|slc| slc.iter()).filter_map(
//             |(gen, val)| {
//                 if gen % 2 == 0 {
//                     Some(val)
//                 } else {
//                     None
//                 }
//             },
//         );

//     let current_chunk_iter = self
//         .current_chunk
//         .iter()
//         .filter_map(Option::as_ref)
//         .filter_map(
//             |(gen, val)| {
//                 if gen % 2 == 0 {
//                     Some(val)
//                 } else {
//                     None
//                 }
//             },
//         );

//     full_chunks_iter.chain(current_chunk_iter)
// }

// /// Construct an iterator over all the values in the slot map as mutable
// /// references
// pub fn iter_mut<'a>(&'a mut self) -> impl Iterator<Item = &'a mut T> {
//     let full_chunks_iter = self
//         .storage
//         .iter_mut()
//         .flat_map(|slc| slc.iter_mut())
//         .filter_map(
//             |(gen, val)| {
//                 if *gen % 2 == 0 {
//                     Some(val)
//                 } else {
//                     None
//                 }
//             },
//         );

//     let current_chunk_iter = self
//         .current_chunk
//         .iter_mut()
//         .filter_map(Option::as_mut)
//         .filter_map(
//             |(gen, val)| {
//                 if *gen % 2 == 0 {
//                     Some(val)
//                 } else {
//                     None
//                 }
//             },
//         );

//     full_chunks_iter.chain(current_chunk_iter)
// }

#[cfg(test)]
mod test {

    use super::*;

    #[derive(Debug, Hash, Clone, Copy)]
    struct TestKey(usize, SlotMapKeyData);

    impl SlotMapKey<usize> for TestKey {
        fn get_slot_map_key_data(&self) -> &SlotMapKeyData {
            &self.1
        }
    }

    impl From<(usize, SlotMapKeyData)> for TestKey {
        fn from(input: (usize, SlotMapKeyData)) -> Self {
            let (p, k) = input;
            TestKey(p, k)
        }
    }

    fn create_test_map() -> SlotMap<TestKey, usize, String> {
        SlotMap::new()
    }

    #[test]
    fn test_crud() {
        let mut map = create_test_map();

        let insertions = SLOT_MAP_CHUNK_SIZE * 10 + SLOT_MAP_CHUNK_SIZE / 2;

        let mut keys = Vec::new();

        for i in 0..insertions {
            keys.push(map.insert(i, format!("{}", i)));
        }

        assert_eq!(map.len(), insertions);

        for k in keys.iter() {
            assert_eq!(map.get(k), Some(&format!("{}", k.0)));
        }

        for k in keys.iter() {
            assert_eq!(map.remove(k), Some(&mut format!("{}", k.0)));
            assert_eq!(map.get(k), None);
        }

        assert_eq!(map.len(), 0);
        assert_eq!(map.queue.len(), insertions);
    }

    #[test]
    fn test_iter() {
        let mut map = create_test_map();

        let insertions = SLOT_MAP_CHUNK_SIZE * 10 + SLOT_MAP_CHUNK_SIZE / 2;

        let mut keys = Vec::new();

        for i in 0..insertions {
            keys.push(map.insert(i, format!("{}", i)));
        }

        let mut counter = 0usize;

        for v in map.iter() {
            assert_eq!(&format!("{}", counter), v);
            counter += 1;
        }

        assert_eq!(insertions, counter);
    }
    #[test]
    fn test_iter_mut() {
        let mut map = create_test_map();

        let insertions = SLOT_MAP_CHUNK_SIZE * 10 + SLOT_MAP_CHUNK_SIZE / 2;

        let mut keys = Vec::new();

        for i in 0..insertions {
            keys.push(map.insert(i, format!("{}", i)));
        }

        let mut counter = 0usize;

        for v in map.iter_mut() {
            *v = format!("{}", (counter * 2) + 1);
            counter += 1;
        }

        for k in keys.iter() {
            assert_eq!(map.get(k), Some(&format!("{}", (k.0 * 2) + 1)));
        }

        assert_eq!(insertions, counter);
    }

    #[test]
    fn test_clear() {
        let mut map = create_test_map();

        let insertions = SLOT_MAP_CHUNK_SIZE * 10 + SLOT_MAP_CHUNK_SIZE / 2;

        let mut keys = Vec::new();

        for i in 0..insertions {
            keys.push(map.insert(i, format!("{}", i)));
        }

        assert_eq!(map.len(), insertions);

        map.clear();

        assert_eq!(map.len(), 0);
        assert_eq!(map.queue.len(), insertions);

        assert_eq!(map.iter().count(), 0);

        for k in keys.iter() {
            assert_eq!(map.get(k), None);
        }
    }
}
