use one_way_slot_map::*;

define_key_type!(UsefulTestKey<usize> : Clone + Copy + Hash + PartialEq);
define_key_type!(TestKey<usize>);

fn create_test_map() -> SlotMap<TestKey, usize, String> {
    SlotMap::new()
}

#[test]
fn test_macro_defined_key_crud() {
    let mut map = create_test_map();

    let insertions = SLOT_MAP_CHUNK_SIZE * 10;

    let mut keys = Vec::new();

    for i in 0..insertions {
        keys.push(map.insert(i, format!("{}", i)));
    }

    assert_eq!(map.len(), insertions);

    for k in keys.iter() {
        assert_eq!(map.get(k), Some(&format!("{}", k.pointer)));
    }

    for k in keys.iter() {
        assert_eq!(map.remove(k), Some(&mut format!("{}", k.pointer)));
        assert_eq!(map.get(k), None);
    }

    assert_eq!(map.len(), 0);
}

#[test]
fn test_without_type_annotations() {
    let mut map = SlotMap::new();

    let key: TestKey = map.insert(0, "Demo!");

    assert_eq!(Some(&"Demo!"), map.get(&key));

    let slot = map.get_mut(&key).unwrap();

    *slot = "Updated!";

    assert_eq!(Some(&mut "Updated!"), map.remove(&key));
}
