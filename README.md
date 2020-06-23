

# One-Way Slot Map

This is an implementation of the slot map data structure similar to [SlotMap](https://github.com/orlp/slotmap) with fewer restrictions and the ability to embed data inside the key objects. The "one-way" moniker for this crate comes from an implementation detail that prevent inserted values from being taken out again unless they are replaced with another instance. Values that are inserted can be referenced, and written to, but ownership of the values remains with the map even after the value is "removed".

The data structure uses fixed size chunks (like [SlotMap's DenseSlotMap](https://docs.rs/slotmap/0.4.0/slotmap/dense/struct.DenseSlotMap.html)), so lookups require 2 steps of indirection.

## Example Usage:

First create a Key Class with an embedded data type
```rust
/// Define a simple key with an embedded usize
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
assert_eq!(None, slot_map.get(&key));
```

## Performance

Benchmarks to come, but in summary, this slot map is about half as fast as fast as the default implementation of [SlotMap's SlotMap](https://docs.rs/slotmap/0.4.0/slotmap/struct.SlotMap.html), slightly faster than [SlotMap's DenseSlotMap](https://docs.rs/slotmap/0.4.0/slotmap/dense/struct.DenseSlotMap.html) and about a dozen times faster than std::collections::HashMap.