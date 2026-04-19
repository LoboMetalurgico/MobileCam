//! Module containing implementations of sparse sets for efficient index-based storage and retrieval of elements.

use parking_lot::Mutex;

/// A sparse set is a data structure that allows for efficient insertion, deletion, and retrieval of elements by index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SparseSet<T> {
    /// The actual data stored in the sparse set.
    data: Vec<T>,
    /// Maps from sparse indices to dense indices.
    sparse: Vec<usize>,
    /// Maps from dense indices to sparse indices.
    dense: Vec<usize>,
    /// A free list to keep track of available indices for reuse.
    free_list: Vec<usize>,
}

impl<T> SparseSet<T> {
    /// Creates a new [`SparseSet`] with the specified capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            data: Vec::with_capacity(capacity),
            sparse: Vec::with_capacity(capacity),
            dense: Vec::with_capacity(capacity),
            free_list: Vec::new(),
        }
    }

    /// Inserts a value into the [`SparseSet`] and returns its index.
    pub fn insert(&mut self, value: T) -> usize {
        let index = if let Some(free_index) = self.free_list.pop() {
            free_index
        } else {
            self.sparse.push(self.data.len());
            self.sparse.len() - 1
        };

        self.data.push(value);
        self.dense.push(index);
        self.sparse[index] = self.data.len() - 1;
        index
    }

    /// Retrieves a value from the [`SparseSet`] by its index, returning an `Option<&T>`.
    pub fn get(&self, index: usize) -> Option<&T> {
        if index < self.sparse.len() {
            let dense_index = self.sparse[index];
            if dense_index < self.data.len() && self.dense[dense_index] == index {
                return Some(&self.data[dense_index]);
            }
        }
        None
    }

    /// Removes a value from the [`SparseSet`] by its index, returning an `Option<T>`.
    pub fn remove(&mut self, index: usize) -> Option<T> {
        if index < self.sparse.len() {
            let dense_index = self.sparse[index];
            if dense_index < self.data.len() && self.dense[dense_index] == index {
                let removed_value = self.data.swap_remove(dense_index);
                let last_dense_index = self.dense.len() - 1;
                if dense_index != last_dense_index {
                    let last_sparse_index = self.dense[last_dense_index];
                    self.sparse[last_sparse_index] = dense_index;
                    self.dense[dense_index] = last_sparse_index;
                }
                self.dense.pop();
                self.free_list.push(index);
                return Some(removed_value);
            }
        }
        None
    }

    /// Returns an iterator over the elements of the [`SparseSet`], yielding pairs of (index, value).
    pub fn iter(&self) -> SparseSetIter<'_, T> {
        SparseSetIter { sparse_set: self, current: 0 }
    }
}

/// A thread-safe wrapper around [`SparseSet`] using [`Mutex`].
#[derive(Debug)]
pub struct SyncSparseSet<T> {
    // The inner [`SparseSet`] is protected by a [`Mutex`] for thread safety.
    inner: Mutex<SparseSet<T>>,
}

impl<T> SyncSparseSet<T> {
    /// Creates a new [`SyncSparseSet`] with the specified capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: Mutex::new(SparseSet::with_capacity(capacity)),
        }
    }

    /// Inserts a value into the [`SparseSet`] and returns its index.
    pub fn insert(&self, value: T) -> usize {
        self.inner.lock().insert(value)
    }

    /// Retrieves a value from the [`SparseSet`] by its index and applies a view function to it, returning an `Option<U>`.
    pub fn view<U, F: FnOnce(&T) -> U>(&self, index: usize, view_fn: F) -> Option<U> {
        self.inner.lock().get(index).map(view_fn)
    }

    /// Removes a value from the [`SparseSet`] by its index, returning an `Option<T>`.
    pub fn remove(&self, index: usize) -> Option<T> {
        self.inner.lock().remove(index)
    }

    /// Returns a vector of mapped values from the [`SparseSet`] by applying the provided mapping function to each element.
    pub fn map<U, F: Fn((usize, &T)) -> U>(&self, map_fn: F) -> Vec<U> {
        self.inner.lock().iter().map(map_fn).collect()
    }
}


/// An iterator over the elements of a [`SparseSet`], yielding pairs of (index, value).
pub struct SparseSetIter<'a, T> {
    /// A reference to the [`SparseSet`] being iterated over.
    sparse_set: &'a SparseSet<T>,
    /// The current index in the iteration.
    current: usize,
}

impl<'a, T> Iterator for SparseSetIter<'a, T> {
    type Item = (usize, &'a T);

    fn next(&mut self) -> Option<Self::Item> {
        while self.current < self.sparse_set.sparse.len() {
            let index = self.current;
            self.current += 1;
            if let Some(value) = self.sparse_set.get(index) {
                return Some((index, value));
            }
        }
        None
    }
}