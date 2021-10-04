use ::rand::seq::SliceRandom;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use one_way_slot_map::define_key_type;
use one_way_slot_map::{SlotMap as OneWay, SlotMapKeyData};
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use slotmap::{DefaultKey, SlotMap};
use std::collections::HashMap;

define_key_type!(BenchKey<()> : Clone + Copy + Hash + PartialEq + Eq);

fn bench_key(number: usize) -> BenchKey {
    BenchKey::from(((), SlotMapKeyData::from(number as u64)))
}

#[allow(dead_code)]
fn random_string(max_len: usize) -> String {
    let mut rng = thread_rng();

    let len = rng.gen_range(0usize..max_len);

    String::from_utf8(
        std::iter::repeat(())
            .map(|()| rng.sample(Alphanumeric))
            .take(len)
            .collect(),
    )
    .expect("Using alphanumeric chars makes for valid u8 strings")
}

fn insert_many_one_way(slot_map: &mut OneWay<BenchKey, (), usize>, n: usize) {
    for i in 0..n {
        let _ = slot_map.insert((), i);
    }
}

fn insert_many_slot_map(slot_map: &mut SlotMap<DefaultKey, usize>, n: usize) {
    for i in 0..n {
        let _ = slot_map.insert(i);
    }
}

fn insert_many_hash_map(map: &mut HashMap<BenchKey, usize>, n: usize) {
    for i in 0..n {
        let _ = map.insert(bench_key(i), i);
    }
}

fn read_many_one_way(
    slot_map: &OneWay<BenchKey, (), usize>,
    keys: &Vec<BenchKey>,
    k: usize,
) {
    for _ in 0..k {
        keys.iter().map(|key| slot_map.get(key)).for_each(|v_opt| {
            let _ = v_opt.unwrap();
        });
    }
}

fn delete_many_one_way(
    slot_map: &mut OneWay<BenchKey, (), usize>,
    keys: &Vec<BenchKey>,
) {
    for key in keys.iter() {
        let _ = slot_map.remove(key).unwrap();
    }
}

fn delete_many_slotmap(
    slot_map: &mut SlotMap<DefaultKey, usize>,
    keys: &Vec<DefaultKey>,
) {
    for key in keys.iter() {
        let _ = slot_map.remove(*key).unwrap();
    }
}

fn read_many_slotmap(
    slot_map: &SlotMap<DefaultKey, usize>,
    keys: &Vec<DefaultKey>,
    k: usize,
) {
    for _ in 0..k {
        keys.iter().map(|key| slot_map.get(*key)).for_each(|v_opt| {
            let _ = v_opt.unwrap();
        });
    }
}

#[allow(dead_code)]
fn read_many_hash_map(
    map: &HashMap<BenchKey, usize>,
    keys: &Vec<BenchKey>,
    k: usize,
) {
    for _ in 0..k {
        keys.iter().map(|key| map.get(key)).for_each(|v_opt| {
            let _ = v_opt.unwrap();
        });
    }
}

fn insertion_benchmark(c: &mut Criterion) {
    c.bench_function("one-way inserting 10k", |b| {
        b.iter(|| {
            let mut map: OneWay<BenchKey, (), usize> = OneWay::new();

            insert_many_one_way(black_box(&mut map), black_box(10_000));
        })
    });
    c.bench_function("slotmap inserting 10k", |b| {
        b.iter(|| {
            let mut map: SlotMap<DefaultKey, usize> = SlotMap::new();

            insert_many_slot_map(black_box(&mut map), black_box(10_000));
        })
    });
    c.bench_function("hashmap inserting 10k", |b| {
        b.iter(|| {
            let mut map: HashMap<BenchKey, usize> = HashMap::new();

            insert_many_hash_map(black_box(&mut map), black_box(10_000));
        })
    });
}

fn read_benchmark(c: &mut Criterion) {
    let mut one_way_keys: Vec<BenchKey> = Vec::new();
    let mut slotmap_keys: Vec<DefaultKey> = Vec::new();

    let mut one_way: OneWay<BenchKey, (), usize> = OneWay::new();
    let mut slot_map: SlotMap<DefaultKey, usize> = SlotMap::new();
    let mut hashmap: HashMap<BenchKey, usize> = HashMap::new();

    for i in 0..10_000 {
        one_way_keys.push(one_way.insert((), i));
        slotmap_keys.push(slot_map.insert(i));
    }

    for (i, k) in one_way_keys.iter().enumerate() {
        hashmap.insert(*k, i);
    }

    let mut rng = thread_rng();

    one_way_keys.shuffle(&mut rng);
    slotmap_keys.shuffle(&mut rng);

    c.bench_function("one-way reading 1m from 10k", |b| {
        b.iter(|| {
            read_many_one_way(&one_way, &one_way_keys, 100);
        })
    });
    c.bench_function("slotmap reading 1m from 10k", |b| {
        b.iter(|| {
            read_many_slotmap(&slot_map, &slotmap_keys, 100);
        })
    });

    // Lol, this is too slow to do every time. 10x slower than one-way
    // c.bench_function("hashmap reading 1m from 10k", |b| {
    //     b.iter(|| {
    //         read_many_hash_map(&hashmap, &one_way_keys, 100);
    //     })
    // });
}

fn deletion_benchmark(c: &mut Criterion) {
    c.bench_function("one-way deleting 1M", |b| {
        b.iter_custom(|_| {
            let mut one_way_keys: Vec<BenchKey> = Vec::new();
            let mut one_way: OneWay<BenchKey, (), usize> = OneWay::new();

            for i in 0..1_000_000 {
                one_way_keys.push(one_way.insert((), i));
            }

            let mut rng = thread_rng();

            one_way_keys.shuffle(&mut rng);

            let start = std::time::Instant::now();

            delete_many_one_way(
                black_box(&mut one_way),
                black_box(&one_way_keys),
            );

            start.elapsed()
        })
    });

    c.bench_function("slotmap deleting 1M", |b| {
        b.iter_custom(|_| {
            let mut keys: Vec<DefaultKey> = Vec::new();
            let mut map: SlotMap<DefaultKey, usize> = SlotMap::new();

            for i in 0..1_000_000 {
                keys.push(map.insert(i));
            }

            let mut rng = thread_rng();

            keys.shuffle(&mut rng);

            let start = std::time::Instant::now();

            delete_many_slotmap(black_box(&mut map), black_box(&keys));

            start.elapsed()
        })
    });
    // c.bench_function("slotmap reading 1m from 10k", |b| {
    //     b.iter(|| {
    //         read_many_slotmap(&slot_map, &slotmap_keys, 100);
    //     })
    // });

    // // Lol, this is too slow to do every time. 10x slower than one-way
    // c.bench_function("hashmap reading 1m from 10k", |b| {
    //     b.iter(|| {
    //         read_many_hash_map(&hashmap, &one_way_keys, 100);
    //     })
    // });
}

criterion_group!(
    benches,
    deletion_benchmark,
    insertion_benchmark,
    read_benchmark,
);
criterion_main!(benches);
