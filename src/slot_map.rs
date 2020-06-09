use super::{SlotMapKey, SlotMapKeyData};
use std::collections::VecDeque;
use std::marker::PhantomData;

/// Size of the individual array chunks in the slot map
pub const SLOT_MAP_CHUNK_SIZE: usize = 256;

// Require the chunk size to be a power of 2
#[cfg(test)]
mod sanity_checks {
    const_assert_eq!(super::SLOT_MAP_CHUNK_SIZE.count_ones(), 1u32);
}
/// Simple and naive map that keeps elements in slots that can be accessed by
/// id and that id is guaranteed not to change over time
pub struct SlotMap<K, P, T>
where
    K: SlotMapKey<P>,
{
    // This will be replaced with Box<[MaybeUninit] when assumeInitRef is stable
    current_chunk: Box<[Option<(usize, T)>; SLOT_MAP_CHUNK_SIZE]>,
    current_chunk_index: usize,
    next_open_slot_in_current_chunk: usize,
    storage: Vec<Box<[(usize, T); SLOT_MAP_CHUNK_SIZE]>>,
    queue: VecDeque<SlotMapKeyData>,

    phantom_k: PhantomData<K>,
    phantom_p: PhantomData<P>,
}

impl<K, P, T> std::fmt::Debug for SlotMap<K, P, T>
where
    T: std::fmt::Debug,
    K: SlotMapKey<P>,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

impl<K, P, T> SlotMap<K, P, T>
where
    K: SlotMapKey<P>,
{
    /// Create a new default simple slot map
    pub fn new() -> SlotMap<K, P, T> {
        SlotMap {
            current_chunk: Box::new(
                array_macro::array![None; SLOT_MAP_CHUNK_SIZE],
            ),
            storage: Vec::new(),
            queue: Default::default(),
            current_chunk_index: 0,
            next_open_slot_in_current_chunk: 0,

            phantom_k: PhantomData::default(),
            phantom_p: PhantomData::default(),
        }
    }

    /// Get the number of items in the slot map
    pub fn len(&self) -> usize {
        (self.storage.len() + 1) * SLOT_MAP_CHUNK_SIZE
            - self.queue.len()
            - (SLOT_MAP_CHUNK_SIZE - self.next_open_slot_in_current_chunk)
    }

    /// Write the given value in the given slot that has already been used
    fn write_to_existing_slot(&mut self, slot_key: &SlotMapKeyData, value: T) {
        if slot_key.chunk_index == self.current_chunk_index {
            if let Some(slot) =
                self.current_chunk.get_mut(slot_key.index_in_chunk)
            {
                slot.replace((slot_key.generation, value));
            } else {
                panic!(
                    "Invalid index {:?} in chunk {} ",
                    slot_key.index_in_chunk, slot_key.chunk_index
                );
            }
        } else {
            if let Some(chunk) = self.storage.get_mut(slot_key.chunk_index) {
                if let Some(slot) = chunk.get_mut(slot_key.index_in_chunk) {
                    *slot = (slot_key.generation, value);
                }
            } else {
                panic!("Invalid chunk index {}", slot_key.chunk_index);
            }
        }
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
        self.current_chunk_index = self.storage.len();
        self.next_open_slot_in_current_chunk = 0;
    }

    /// Insert the given value with the given pointer into the next new slot
    fn write_to_new_slot(&mut self, pointer: P, value: T) -> K {
        if let Some(slot) = self
            .current_chunk
            .get_mut(self.next_open_slot_in_current_chunk)
        {
            slot.replace((0, value));
        } else {
            panic!(
                "Invalid chunk index {} in current chunk ",
                self.next_open_slot_in_current_chunk
            );
        }

        let index_in_chunk = self.next_open_slot_in_current_chunk;
        let chunk_index = self.current_chunk_index;
        self.next_open_slot_in_current_chunk =
            (self.next_open_slot_in_current_chunk + 1) % SLOT_MAP_CHUNK_SIZE;

        if self.next_open_slot_in_current_chunk == 0 {
            self.move_current_chunk_to_storage();
        }

        K::from((pointer, SlotMapKeyData::new(index_in_chunk, chunk_index, 0)))
    }

    /// insert the given item into the slot map and return its key
    pub fn insert(&mut self, pointer: P, value: T) -> K {
        // First try to insert at the next available location given in the
        // queue
        match self.queue.pop_front() {
            Some(mut empty_slot_key) => {
                empty_slot_key.increment_generation();

                self.write_to_existing_slot(&empty_slot_key, value);

                K::from((pointer, empty_slot_key))
            }
            None => self.write_to_new_slot(pointer, value),
        }
    }

    /// Panic if the given slot key contains invalid data (index out of bounds)
    fn validate_slot_key(&self, key: &K) {
        let ref key_data = key.get_slot_map_key_data();

        if key_data.chunk_index > self.current_chunk_index {
            panic!(
                "Chunk index is greater than number of chunks in key {:?}",
                key_data
            );
        }

        if key_data.index_in_chunk > SLOT_MAP_CHUNK_SIZE {
            panic!(
                "Index in chunk is greater than chunk size in key {:?}",
                key_data
            );
        }

        if key_data.chunk_index == self.current_chunk_index
            && key_data.index_in_chunk >= self.next_open_slot_in_current_chunk
        {
            panic!(
                "Index in chunk is greater than size of the used portion of \
                the current chunk in key {:?}",
                key_data
            );
        }
    }

    /// Get the value for the given key if there is one
    pub fn get(&self, key: &K) -> Option<&T> {
        let key_data = key.get_slot_map_key_data();

        // This allows us to unwrap instead of matching and panicking
        self.validate_slot_key(key);

        let (generation, value) =
            if key_data.chunk_index == self.current_chunk_index {
                self.current_chunk
                    .get(key_data.index_in_chunk)
                    .unwrap()
                    .as_ref()
                    .unwrap()
            } else {
                self.storage
                    .get(key_data.chunk_index)
                    .unwrap()
                    .get(key_data.index_in_chunk)
                    .unwrap()
            };

        if key_data.generation == *generation {
            Some(value)
        } else {
            None
        }
    }

    /// Get the value for the given key if there is one
    pub fn get_mut(&mut self, key: &K) -> Option<&mut T> {
        let key_data = key.get_slot_map_key_data();

        // This allows us to unwrap instead of matching and panicking
        self.validate_slot_key(key);

        let (generation, value) =
            if key_data.chunk_index == self.current_chunk_index {
                self.current_chunk
                    .get_mut(key_data.index_in_chunk)
                    .unwrap()
                    .as_mut()
                    .unwrap()
            } else {
                self.storage
                    .get_mut(key_data.chunk_index)
                    .unwrap()
                    .get_mut(key_data.index_in_chunk)
                    .unwrap()
            };

        if key_data.generation == *generation {
            Some(value)
        } else {
            None
        }
    }
    /// Remove the item at the given index and return true iff successful
    pub fn remove(&mut self, key: &K) -> Option<&mut T> {
        let key_data = key.get_slot_map_key_data();

        // This allows us to unwrap instead of matching and panicking
        self.validate_slot_key(key);

        let (generation, value) =
            if key_data.chunk_index == self.current_chunk_index {
                self.current_chunk
                    .get_mut(key_data.index_in_chunk)
                    .unwrap()
                    .as_mut()
                    .unwrap()
            } else {
                self.storage
                    .get_mut(key_data.chunk_index)
                    .unwrap()
                    .get_mut(key_data.index_in_chunk)
                    .unwrap()
            };

        if *generation == key_data.generation {
            *generation += 1;
            self.queue.push_front(*key_data);
            Some(value)
        } else {
            None
        }
    }

    /// Clears all the values in the slot map
    pub fn clear(&mut self) {
        self.storage
            .iter_mut()
            .flat_map(|chunk| chunk.iter_mut())
            .for_each(|(gen, _)| *gen += 1);

        self.current_chunk.iter_mut().for_each(|slot| *slot = None);
    }

    /// Construct an iterator over all the values in the slot map
    pub fn iter<'a>(&'a self) -> impl Iterator<Item = &'a T> {
        let full_chunks_iter =
            self.storage.iter().flat_map(|slc| slc.iter()).filter_map(
                |(gen, val)| {
                    if gen % 2 == 1 {
                        Some(val)
                    } else {
                        None
                    }
                },
            );

        let current_chunk_iter = self
            .current_chunk
            .iter()
            .filter_map(Option::as_ref)
            .filter_map(
                |(gen, val)| {
                    if gen % 2 == 1 {
                        Some(val)
                    } else {
                        None
                    }
                },
            );

        full_chunks_iter.chain(current_chunk_iter)
    }

    /// Construct an iterator over all the values in the slot map as mutable
    /// references
    pub fn iter_mut<'a>(&'a mut self) -> impl Iterator<Item = &'a mut T> {
        let full_chunks_iter = self
            .storage
            .iter_mut()
            .flat_map(|slc| slc.iter_mut())
            .filter_map(
                |(gen, val)| {
                    if *gen % 2 == 1 {
                        Some(val)
                    } else {
                        None
                    }
                },
            );

        let current_chunk_iter = self
            .current_chunk
            .iter_mut()
            .filter_map(Option::as_mut)
            .filter_map(
                |(gen, val)| {
                    if *gen % 2 == 1 {
                        Some(val)
                    } else {
                        None
                    }
                },
            );

        full_chunks_iter.chain(current_chunk_iter)
    }
}

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

        let insertions = SLOT_MAP_CHUNK_SIZE * 10;

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
}
