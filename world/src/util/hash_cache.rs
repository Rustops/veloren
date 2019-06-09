use std::hash::Hash;
use fxhash::FxHashMap;

pub struct HashCache<K: Hash + Eq + Clone, V> {
    capacity: usize,
    map: FxHashMap<K, (usize, V)>,
    counter: usize,
}

impl<K: Hash + Eq + Clone, V> Default for HashCache<K, V> {
    fn default() -> Self {
        Self::with_capacity(1024)
    }
}

impl<K: Hash + Eq + Clone, V> HashCache<K, V> {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            capacity,
            map: FxHashMap::default(),
            counter: 0,
        }
    }

    pub fn maintain(&mut self) {
        const CACHE_BLOAT_RATE: usize = 2;

        if self.map.len() > self.capacity * CACHE_BLOAT_RATE {
            let (capacity, counter) = (self.capacity, self.counter);
            self.map.retain(|_, (c, _)| *c + capacity > counter);
        }
    }

    pub fn get<F: FnOnce(K) -> V>(&mut self, key: K, f: F) -> &V {
        self.maintain();

        let counter = &mut self.counter;
        &self
            .map
            .entry(key.clone())
            .or_insert_with(|| {
                *counter += 1;
                (*counter, f(key))
            })
            .1
    }
}
