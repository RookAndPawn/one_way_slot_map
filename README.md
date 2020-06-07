

# One-Way Slot Map

This is an implementation of the slot map data struture similar to [SlotMap](https://github.com/orlp/slotmap) with fewer restrictions and the ability to embed data inside the key objects. The "one-way" moniker for this crate comes from an implementaiton detail that prevernt inserted values from being taken out againe unless they are replaced with another instance. Values that are inserted can be referenced, and written to, but ownership of the values remains with the map even after the value is "removed".

The data structure uses fixed size chunks (like [SlotMap's DenseSlotMap](https://docs.rs/slotmap/0.4.0/slotmap/dense/struct.DenseSlotMap.html)), so lookups require 2 steps of indirection.

Example Usage:

First create a Key Class with an embedded data type
```rust
/// Define a simple key with an embedded uszie
define_key_type!(DemoKey<usize>);

/// Or define a less-simple key with some derived traits
define_key_type!(TestKeyWithDerives<usize> : Copy + Clone + Debug);
```

Then create a slot map and use the key for crud operations

```rust
let mut slot_map = SlotMap::new();

let key: DemoKey = slot_map.insert(0, "Demo!");

assert_eq!(Some(&"Demo!"), slot_map.get(&key));

let slot = slot_map.get_mut(&key).unwrap();

*slot = "Updated!";

assert_eq!(Some(&mut "Updated!"), slot_map.remove(&key));
```