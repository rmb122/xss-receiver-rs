use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A multimap data structure that maps keys to multiple values.
/// Internally uses BTreeMap<K, Vec<V>> for ordered storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MultiMap<K, V>
where
    K: Ord + Clone,
    V: Clone,
{
    inner: BTreeMap<K, Vec<V>>,
}

impl<K, V> MultiMap<K, V>
where
    K: Ord + Clone,
    V: Clone,
{
    /// Creates a new empty MultiMap.
    pub fn new() -> Self {
        Self {
            inner: BTreeMap::new(),
        }
    }

    /// Inserts a key-value pair into the multimap.
    /// If the key already exists, the value is appended to the existing Vec.
    /// If the key doesn't exist, a new Vec is created.
    pub fn insert(&mut self, key: K, value: V) {
        self.inner.entry(key).or_insert_with(Vec::new).push(value);
    }

    /// Gets the first value associated with the given key.
    /// Returns None if the key doesn't exist or the Vec is empty.
    pub fn get(&self, key: &K) -> Option<&V> {
        self.inner.get(key).and_then(|vec| vec.first())
    }

    pub fn get_all(&self, key: &K) -> Option<&Vec<V>> {
        self.inner.get(key)
    }

    /// Returns an iterator over all key-value pairs in the multimap.
    /// For keys with multiple values, each value is returned as a separate pair.
    ///
    /// # Example
    /// If the multimap contains:
    /// - key1 -> [val1, val2]
    /// - key2 -> [val3]
    ///
    /// The iterator will yield:
    /// - (&key1, &val1)
    /// - (&key1, &val2)
    /// - (&key2, &val3)
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.inner
            .iter()
            .flat_map(|(k, values)| values.iter().map(move |v| (k, v)))
    }
}

impl<K, V> Default for MultiMap<K, V>
where
    K: Ord + Clone,
    V: Clone,
{
    fn default() -> Self {
        Self::new()
    }
}
