use super::{SlotMapKey, SlotMapKeyData};
use std::borrow::Borrow;
use std::marker::PhantomData;
use array_macro::array;

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

    #[allow(clippy::vec_box)]
    filled_chunks: Vec<Box<[(SlotMapKeyData, T); SLOT_MAP_CHUNK_SIZE]>>,
    current_chunk_index: u32,
    current_chunk_cursor: u16,
}

impl<T> Slots<T> {
    pub fn new() -> Slots<T> {
        Slots {
            current_chunk: Box::new(
                array![None; SLOT_MAP_CHUNK_SIZE],
            ),
            filled_chunks: Vec::new(),
            current_chunk_index: Default::default(),
            current_chunk_cursor: Default::default(),
        }
    }

    fn get_slot(&self, key: &SlotMapKeyData) -> Option<&(SlotMapKeyData, T)> {
        if key.chunk_index < self.current_chunk_index {
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

    /// Move the current chunk into filled chunks
    fn move_current_chunk_to_filled_chunk(&mut self) {
        let storage_chunk = Box::new(array_macro::array![|i| {
                self.current_chunk
                    .get_mut(i)
                    .expect("Expected correctly sized chunk")
                    .take()
                    .expect("Expected all slots in current chunk to be filled")
            } ; SLOT_MAP_CHUNK_SIZE]);

        self.filled_chunks.push(storage_chunk);
        self.current_chunk_index = self.filled_chunks.len() as u32;
        self.current_chunk_cursor = 0;
    }

    /// Construct an iterator over all initialized slots
    pub fn iter(&self) -> impl Iterator<Item = &(SlotMapKeyData, T)> {
        let full_chunks_iter =
            self.filled_chunks.iter().flat_map(|slc| slc.iter());

        let current_chunk_iter = self
            .current_chunk
            .iter()
            .take(self.current_chunk_cursor as usize)
            .filter_map(Option::as_ref);

        full_chunks_iter.chain(current_chunk_iter)
    }

    /// Construct an iterator over all initialized slots as mutable references
    pub fn iter_mut(
        &mut self,
    ) -> impl Iterator<Item = &mut (SlotMapKeyData, T)> {
        let full_chunks_iter =
            self.filled_chunks.iter_mut().flat_map(|slc| slc.iter_mut());

        let current_chunk_iter = self
            .current_chunk
            .iter_mut()
            .take(self.current_chunk_cursor as usize)
            .filter_map(Option::as_mut);

        full_chunks_iter.chain(current_chunk_iter)
    }

    /// Create new slots based on this one with the values mapped with the given
    /// function
    fn map<R>(&self, mut mapper: impl FnMut(&T) -> R) -> Slots<R> {
        Slots {
            current_chunk: Box::new(
                array_macro::array![|i| {
                    let slot_opt = self.current_chunk.get(i).unwrap();
                    slot_opt.as_ref().map(|slot| (slot.0, mapper(&slot.1)))
                }; SLOT_MAP_CHUNK_SIZE],
            ),
            filled_chunks: self.filled_chunks.iter().map(|chunk| {
                Box::new(array!(|i| {
                    let slot = chunk.get(i).unwrap();
                    (slot.0, mapper(&slot.1))
                }; SLOT_MAP_CHUNK_SIZE))
            })
            .collect(),
            current_chunk_index: self.current_chunk_index,
            current_chunk_cursor: self.current_chunk_cursor,
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

impl<K, P, T> std::fmt::Debug for SlotMap<K, P, T>
where
    T: std::fmt::Debug,
    K: SlotMapKey<P>,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

impl<K, P, T> Default for SlotMap<K, P, T>
where
    K: SlotMapKey<P>,
{
    fn default() -> Self {
        SlotMap::new()
    }
}

impl<K, P, T> SlotMap<K, P, T>
where
    K: SlotMapKey<P>,
{
    /// Create a new default simple slot map
    pub fn new() -> SlotMap<K, P, T> {
        SlotMap {
            slots: Slots::new(),
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

    /// Tells if this map is empty
    ///
    /// ```
    /// # use one_way_slot_map::*;
    /// # define_key_type!(TestKey<()>);
    /// let mut map = SlotMap::<TestKey,(),usize>::new();
    ///
    /// assert_eq!(true, map.is_empty());
    ///
    /// let _ = map.insert((),10);
    /// let _ = map.insert((),42);
    ///
    /// assert_eq!(false, map.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// insert the given item into the slot map and return its key
    ///
    /// ```
    /// # use one_way_slot_map::*;
    /// # use std::borrow::Borrow;
    /// define_key_type!(TestKey<String>);
    /// let mut map = SlotMap::<TestKey,String,usize>::new();
    ///
    /// let key = map.insert("My Key".to_owned(), 10);
    /// assert_eq!("My Key", key.pointer);
    /// assert_eq!(&SlotMapKeyData::from(0), key.borrow());
    /// ```
    pub fn insert(&mut self, pointer: P, value: T) -> K {
        let next_slot = &mut self.next_open_slot;

        let key_data = if next_slot.chunk_index < self.slots.current_chunk_index
            || next_slot.index_in_chunk < self.slots.current_chunk_cursor
        {
            let (new_next_slot, old_val) = self
                .slots
                .get_existing_slot_mut(next_slot)
                .expect("invalid next slot pointer");
            *old_val = value;
            new_next_slot.increment_generation();
            new_next_slot.swap_coordinates(next_slot);
            *new_next_slot
        } else {
            let key_data = *next_slot;
            let slot_opt = self.slots.get_current_chunk_slot_mut(next_slot);
            *slot_opt = Some((*next_slot, value));
            if self.next_open_slot.increment_coordinates() {
                self.slots.move_current_chunk_to_filled_chunk()
            } else {
                self.slots.current_chunk_cursor += 1;
            }
            key_data
        };

        self.len += 1;

        K::from((pointer, key_data))
    }

    /// Get a reference to the item in the map that corresponds to the given key
    /// if it exists
    ///
    /// ```
    /// # use one_way_slot_map::*;
    /// define_key_type!(TestKey<()>);
    /// let mut map = SlotMap::<TestKey,(),&'static str>::new();
    ///
    /// let key = map.insert((), "Hello!");
    ///
    /// assert_eq!(Some(&"Hello!"), map.get(&key));
    ///
    /// // Create a key that won't be in the map. This is non-ergonomic because
    /// // it's not really a use case we expect,
    /// let fake_key = TestKey::from(((), SlotMapKeyData::from(1u64)));
    ///
    /// assert_eq!(None, map.get(&fake_key));
    /// ```
    pub fn get(&self, key: &K) -> Option<&T> {
        self.get_unbounded(key)
    }

    /// Same as get method, but doesn't restrict input key to the type bound
    /// to this map. This method isn't unsafe; it just doesn't prevent you from
    /// getting data with a key of the wrong type
    pub fn get_unbounded(
        &self,
        key: &impl Borrow<SlotMapKeyData>,
    ) -> Option<&T> {
        let key_data = key.borrow();

        self.slots
            .get_slot(key_data)
            .filter(|slot| slot.0.is_filled())
            .filter(|slot| slot.0.generation == key_data.generation)
            .map(|slot| &slot.1)
    }

    /// Get a mutable reference to the item in the map that corresponds to the
    /// given key if it exists
    ///
    /// ```
    /// # use one_way_slot_map::*;
    /// define_key_type!(TestKey<()>);
    /// let mut map = SlotMap::<TestKey,(),&'static str>::new();
    ///
    /// let key = map.insert((), "Hello!");
    ///
    /// {
    ///     if let Some(item) = map.get_mut(&key) {
    ///         *item = "World?";
    ///     }
    /// }
    /// assert_eq!(Some(&"World?"), map.get(&key));
    ///
    /// // Create a key that won't be in the map. This is non-ergonomic because
    /// // it's not really a use case we expect,
    /// let fake_key = TestKey::from(((), SlotMapKeyData::from(1u64)));
    ///
    /// assert_eq!(None, map.get_mut(&fake_key));
    /// ```
    pub fn get_mut(&mut self, key: &K) -> Option<&mut T> {
        self.get_mut_unbounded(key)
    }

    /// Same as get_mut method, but doesn't restrict input key to the type bound
    /// to this map. This method isn't unsafe; it just doesn't prevent you from
    /// writing data with a key of the wrong type
    pub fn get_mut_unbounded(
        &mut self,
        key: &impl Borrow<SlotMapKeyData>,
    ) -> Option<&mut T> {
        let key_data = key.borrow();

        self.slots
            .get_existing_slot_mut(key_data)
            .filter(|slot| slot.0.is_filled())
            .filter(|slot| slot.0.generation == key_data.generation)
            .map(|slot| &mut slot.1)
    }

    /// Remove the item at the given index and return a mutable ref to the
    /// item removed if there was one
    ///
    /// ```
    /// # use one_way_slot_map::*;
    /// # define_key_type!(TestKey<()>);
    /// let mut map = SlotMap::<TestKey,(),&'static str>::new();
    ///
    /// let key = map.insert((), "Hello!");
    ///
    /// assert!(map.get(&key).is_some());
    ///
    /// assert_eq!(Some(&mut "Hello!"), map.remove(&key));
    ///
    /// assert_eq!(None, map.get(&key));
    /// ```
    pub fn remove(&mut self, key: &K) -> Option<&mut T> {
        self.remove_unbounded(key)
    }

    /// Same as remove method, but doesn't restrict input key to the type bound
    /// to this map. This method isn't unsafe; it just doesn't prevent you from
    /// writing data with a key of the wrong type
    pub fn remove_unbounded(
        &mut self,
        key: &impl Borrow<SlotMapKeyData>,
    ) -> Option<&mut T> {
        let key_data = key.borrow();

        if let Some((key, value)) = self
            .slots
            .get_existing_slot_mut(key_data)
            .filter(|(key, _)| key.is_filled())
            .filter(|(key, _)| key.generation == key_data.generation)
        {
            self.len -= 1;
            key.increment_generation();
            key.swap_coordinates(&mut self.next_open_slot);
            Some(value)
        } else {
            None
        }
    }

    /// Check to see if the given key is still valid in this map
    ///
    /// ```
    /// # use one_way_slot_map::*;
    /// define_key_type!(TestKey<()>);
    /// let mut map = SlotMap::<TestKey,(),&'static str>::new();
    ///
    /// let key = map.insert((), "Hello!");
    ///
    /// assert!(map.contains_key(&key));
    ///
    /// // Create a key that won't be in the map. This is non-ergonomic because
    /// // it's not really a use case we expect,
    /// let fake_key = TestKey::from(((), SlotMapKeyData::from(1u64)));
    ///
    /// assert!(!map.contains_key(&fake_key));
    /// ```
    pub fn contains_key(&self, key: &K) -> bool {
        self.contains_key_unbounded(key)
    }

    /// Same as contains_key method, but doesn't restrict input key to the type
    /// bound to this map. This method isn't unsafe; it just doesn't prevent you
    /// from getting data with a key of the wrong type
    pub fn contains_key_unbounded(
        &self,
        key: &impl Borrow<SlotMapKeyData>,
    ) -> bool {
        let key_data = key.borrow();

        self.slots
            .get_slot(key_data)
            .filter(|(existing_key, _)| {
                existing_key.generation == key_data.generation
            })
            .is_some()
    }

    /// Remove all items from this map and process them one-by-one
    pub fn drain(&mut self) -> impl Iterator<Item = &mut T> {
        let len = &mut self.len;
        let next_open_slot = &mut self.next_open_slot;

        Drain {
            inner: self
                .slots
                .iter_mut()
                .filter(|(key, _)| key.is_filled())
                .map(move |(key, val)| {
                    *len -= 1;

                    key.increment_generation();
                    next_open_slot.swap_coordinates(key);

                    val
                }),
            phantom_t: Default::default(),
        }
    }

    /// Clears all the values in the slot map.  This can be a memory intensive
    /// operation because we will have to write information for every non-empty
    /// slot into the queue of slots that can now be used
    pub fn clear(&mut self) {
        let _ = self.drain();
    }

    /// Create an iterator over all items in the items in the map
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.slots
            .iter()
            .filter(|(key, _)| key.is_filled())
            .map(|(_, value)| value)
    }

    /// Construct an iterator over all the values in the slot map as mutable
    /// references
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.slots
            .iter_mut()
            .filter(|(key, _)| key.is_filled())
            .map(|(_, value)| value)
    }

    /// Create a new map that has the same struture as this one, but with the
    /// values mapped with the given closure
    pub fn map<F, R>(&self, mapper: F) -> SlotMap<K, P, R> where F: FnMut(&T) -> R {
        SlotMap {
            slots: self.slots.map(mapper),
            len: self.len,
            next_open_slot: self.next_open_slot,
            phantom_k: Default::default(),
            phantom_p: Default::default()
        }
    }
}

impl<K, P, T> Clone for SlotMap<K, P, T>
where
    K: SlotMapKey<P>,
    T: Clone,
{
    fn clone(&self) -> Self {
        self.map(T::clone)
    }
}

struct Drain<'a, I, T>
where
    I: Iterator<Item = &'a mut T>,
    T: 'a,
{
    inner: I,

    phantom_t: PhantomData<T>,
}

impl<'a, I, T> Iterator for Drain<'a, I, T>
where
    I: Iterator<Item = &'a mut T>,
{
    type Item = &'a mut T;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

impl<'a, I, T> Drop for Drain<'a, I, T>
where
    I: Iterator<Item = &'a mut T>,
{
    /// When the drain is dropped, we just need to ensure any un-iterated items
    /// are processed and thus removed correctly form the map
    fn drop(&mut self) {
        self.for_each(|_| {})
    }
}

#[cfg(test)]
mod test {

    use super::*;
    use rand::seq::SliceRandom;
    use rand::thread_rng;

    #[derive(Debug, Hash, Clone, Copy)]
    struct TestKey(usize, SlotMapKeyData);

    impl Borrow<SlotMapKeyData> for TestKey {
        fn borrow(&self) -> &SlotMapKeyData {
            &self.1
        }
    }

    impl From<(usize, SlotMapKeyData)> for TestKey {
        fn from(input: (usize, SlotMapKeyData)) -> Self {
            let (p, k) = input;
            TestKey(p, k)
        }
    }

    impl SlotMapKey<usize> for TestKey {}

    fn create_test_map() -> SlotMap<TestKey, usize, String> {
        SlotMap::new()
    }

    #[test]
    fn test_crud() {
        let mut map = create_test_map();

        let key = map.insert(0, "0".to_owned());

        assert_eq!(map.len(), 1);

        assert_eq!(map.get(&key), Some(&"0".to_owned()));

        {
            let v = map.get_mut(&key).expect("Key should be present");
            *v = "1".to_owned();
        }

        assert_eq!(map.remove(&key), Some(&mut "1".to_owned()));
        assert_eq!(map.get(&key), None);

        assert_eq!(map.len(), 0);

        map.iter().for_each(|v| {
            dbg!(v);
        });
    }

    #[test]
    fn test_lots_of_crud() {
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

        assert_eq!(map.iter().count(), 0);

        for k in keys.iter() {
            assert_eq!(map.get(k), None);
        }
    }

    fn assert_coordinates_eq(k1: &SlotMapKeyData, k2: &SlotMapKeyData) {
        assert_eq!(k1.chunk_index, k2.chunk_index);
        assert_eq!(k1.index_in_chunk, k2.index_in_chunk);
    }

    #[test]
    fn test_embedded_empty_stack_consistency() {
        let mut map = create_test_map();

        let insertions = SLOT_MAP_CHUNK_SIZE * 10 + SLOT_MAP_CHUNK_SIZE / 2;
        let iterations = 50;

        let mut rng = thread_rng();

        for j in 0..iterations {
            let mut keys = Vec::new();

            for i in 0..insertions {
                let prev_next_slot = map.next_open_slot;

                let next_next_slot =
                    map.slots.get_slot(&prev_next_slot).map(|(key, _)| *key);

                keys.push(map.insert(i, format!("{}", i)));
                assert_coordinates_eq(
                    &prev_next_slot,
                    &map.slots.get_slot(&keys.get(i).unwrap().1).unwrap().0,
                );

                if j > 0 {
                    assert_coordinates_eq(
                        next_next_slot.as_ref().unwrap(),
                        &map.next_open_slot,
                    );
                }
            }

            assert_eq!(map.len(), insertions);
            assert_eq!(map.slots.filled_chunks.len(), 10);
            assert_eq!(
                map.slots.current_chunk_cursor as usize,
                SLOT_MAP_CHUNK_SIZE / 2
            );

            map.slots.iter().enumerate().for_each(|(num, (key, _))| {
                assert_eq!(key.generation, j * 2);
                assert_eq!(
                    key.index_in_chunk as usize,
                    num % SLOT_MAP_CHUNK_SIZE
                );
                assert_eq!(key.chunk_index as usize, num / SLOT_MAP_CHUNK_SIZE);
            });

            assert_eq!(
                SlotMapKeyData::from(insertions as u64),
                map.next_open_slot
            );

            if j % 2 == 0 {
                keys.shuffle(&mut rng);

                for k in keys.drain(..) {
                    let prev_next_slot = map.next_open_slot;
                    assert_eq!(&format!("{}", k.0), map.remove(&k).unwrap());
                    assert_coordinates_eq(&k.1, &map.next_open_slot);

                    let cleared_slot = map.slots.get_slot(&k.1).unwrap().0;

                    assert_coordinates_eq(&prev_next_slot, &cleared_slot);

                    assert_eq!(2 * j + 1, cleared_slot.generation);
                }
            } else {
                map.clear();
            }
        }
    }

    #[test]
    fn test_clone() {
        let mut map = create_test_map();

        let insertions = SLOT_MAP_CHUNK_SIZE * 10 + SLOT_MAP_CHUNK_SIZE / 2;
        let iterations = 50;

        let mut keys = Vec::new();

        for _ in 0..iterations {
            keys.clear();

            for i in 0..insertions {
                keys.push(map.insert(i, format!("{}", i)));
            }

            map.clear();
        }

        let map2 = map.clone();

        map.slots
            .iter()
            .zip(map2.slots.iter())
            .for_each(|(left, right)| {
                assert_eq!(left, right);
            })
    }
}
