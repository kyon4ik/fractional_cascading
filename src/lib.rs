//! Fractional cascading over a fixed collection of sorted slices.
//!
//! Given `k` sorted slices, [`FCSearcher`] answers "how many elements are
//! strictly less than `x` in each slice?" in O(log n + k) time after O(n)
//! preprocessing, compared to O(k log n) for naive repeated binary search.
//!
//! ```
//! use fractional_cascading::FCSearcher;
//!
//! let sources = vec![vec![1, 3, 6, 10], vec![2, 4, 5, 7, 8, 9]];
//! let searcher = FCSearcher::new(&sources);
//!
//! // Number of elements strictly less than 6 in each source, in source order.
//! assert_eq!(searcher.search(&6), [2, 3]);
//!
//! // Same result via the lazy iterator (yields in reverse source order).
//! let rev: Vec<_> = searcher.search_iter(&6).collect();
//! assert_eq!(rev, [3, 2]);
//! ```

use num_traits::Bounded;
use std::borrow::Borrow;
use std::iter::FusedIterator;

/// Fractional cascading structure over a fixed collection of sorted slices.
///
/// Answers "how many elements are strictly less than `key` in each slice?"
/// in O(log n + k) time after O(n) preprocessing, where `n` is the total
/// number of elements and `k` is the number of slices.
#[derive(Clone, Debug)]
pub struct FCSearcher<T> {
    cats: Vec<Vec<Node<T>>>,
}

// ncur -> number of items less than value in current
// nnxt -> number of items less than value in next
#[derive(Clone, Copy, Debug)]
struct Node<T> {
    val: T,
    ncur: usize,
    nnxt: usize,
}

impl<T: Clone + Ord + Bounded> FCSearcher<T> {
    /// Build a `FCSearcher` from an iterator of sorted slices.
    ///
    /// Each source slice must be sorted in ascending order; behaviour is
    /// unspecified if this invariant is violated.
    pub fn new<'slc, I, S>(sources: I) -> Self
    where
        S: Borrow<[T]> + 'slc,
        I: IntoIterator<Item = &'slc S>,
    {
        let mut cats = Vec::new();
        let mut srcs = sources.into_iter();

        if let Some(first_src) = srcs.next() {
            let mut last_cat = cat_from_src(first_src.borrow());

            for src in srcs {
                let new_last_cat = cat_merged_with_src(&last_cat, src.borrow());
                cats.push(last_cat);
                last_cat = new_last_cat;
            }

            cats.push(last_cat);
        }
        cats.reverse();

        Self { cats }
    }

    /// Return a lazy iterator that yields, for each source slice in **reverse** order
    /// (last to first), the number of elements strictly less than `key`.
    ///
    /// Use [`search`](Self::search) to get results in source order.
    pub fn search_iter<'s, 'k>(&'s self, key: &'k T) -> FcIter<'s, 'k, T> {
        FcIter::new(&self.cats, key)
    }

    /// Return the number of elements strictly less than `key` in each source slice,
    /// in source order (first to last).
    #[must_use]
    pub fn search(&self, key: &T) -> Vec<usize> {
        let mut res: Vec<_> = self.search_iter(key).collect();
        res.reverse();
        res
    }
}

/// Lazy iterator returned by [`FCSearcher::search_iter`].
///
/// Yields the number of elements strictly less than the search key in each
/// source slice, in reverse source order (last slice first).
pub struct FcIter<'a, 'k, T> {
    key: &'k T,
    node: Option<&'a Node<T>>,
    cats: core::slice::Iter<'a, Vec<Node<T>>>,
}

impl<'a, 'k, T: Ord> FcIter<'a, 'k, T> {
    fn new(cats: &'a [Vec<Node<T>>], key: &'k T) -> Self {
        if let Some((first_cat, cats)) = cats.split_first() {
            let pos = first_cat.partition_point(|node| node.val < *key);

            // Safety: pos < first_cat.len()
            let node = unsafe { first_cat.get_unchecked(pos) };
            Self {
                key,
                node: Some(node),
                cats: cats.iter(),
            }
        } else {
            Self {
                key,
                node: None,
                cats: cats.iter(),
            }
        }
    }
}

impl<T: Ord> Iterator for FcIter<'_, '_, T> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        let cur = self.node.take()?;

        if let Some(cat) = self.cats.next() {
            // Safety: cur.nnxt < cat.len()
            let nxt = unsafe { cat.get_unchecked(cur.nnxt) };

            if cur.nnxt > 0 {
                // Safety: cur.nnxt > 0
                let before_nxt = unsafe { cat.get_unchecked(cur.nnxt - 1) };
                self.node = if before_nxt.val >= *self.key {
                    Some(before_nxt)
                } else {
                    Some(nxt)
                };
            } else {
                self.node = Some(nxt);
            }
        }

        Some(cur.ncur)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.cats.len() + 1;
        (len, Some(len))
    }
}

impl<T: Ord> FusedIterator for FcIter<'_, '_, T> {}

fn cat_from_src<T: Clone + Eq + Bounded>(src: &[T]) -> Vec<Node<T>> {
    let mut res = Vec::with_capacity(src.len() + 1);

    // Number of elements less than item
    let mut num_less = 0;

    for (index, item) in src.iter().cloned().enumerate() {
        // Safety: num_less <= index < src.len()
        if item != *unsafe { src.get_unchecked(num_less) } {
            num_less = index;
        }
        res.push(Node::new(item, num_less, 0));
    }

    res.push(Node::max(src.len(), 0));
    res
}

fn cat_merged_with_src<T: Clone + Ord + Bounded>(cat: &[Node<T>], src: &[T]) -> Vec<Node<T>> {
    let mut res = Vec::with_capacity((cat.len() >> 1) + src.len() + 1);
    let mut sprev = 0;
    let mut cprev = 0;
    let mut sind = 0;

    for cind in 0..cat.len() - 1 {
        while sind < src.len() && src[sind] < cat[cind].val {
            if src[sind] != src[sprev] {
                sprev = sind;
            }

            if cat[cprev].val == src[sind] {
                res.push(Node::new(src[sind].clone(), sprev, cprev));
            } else {
                res.push(Node::new(src[sind].clone(), sprev, cind));
            }
            sind += 1;
        }

        if cat[cind].val != cat[cprev].val {
            cprev = cind;
        }

        if cind & 1 == 0 {
            res.push(Node::new(cat[cind].val.clone(), sind, cprev));
        }
    }

    while sind < src.len() {
        if src[sind] != src[sprev] {
            sprev = sind;
        }

        if cat[cprev].val == src[sind] {
            res.push(Node::new(src[sind].clone(), sprev, cprev));
        } else {
            res.push(Node::new(src[sind].clone(), sprev, cat.len() - 1));
        }
        sind += 1;
    }

    res.push(Node::max(sind, cat.len() - 1));
    res
}

impl<T> Node<T> {
    fn new(val: T, ncur: usize, nnxt: usize) -> Self {
        Self { val, ncur, nnxt }
    }
}

impl<T: Bounded> Node<T> {
    fn max(ncur: usize, nnxt: usize) -> Self {
        Self {
            val: T::max_value(),
            ncur,
            nnxt,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_catalogs() {
        let catalogs: Vec<Vec<_>> = Vec::new();
        let searcher = FCSearcher::new(&catalogs);
        assert_eq!(searcher.search(&0), &[]);
    }

    #[test]
    fn empty_catalogs() {
        let catalogs = vec![[], [], []];
        let searcher = FCSearcher::new(&catalogs);

        for key in 0..5 {
            assert_eq!(searcher.search(&key), &[0, 0, 0], "key: {key}");
        }
    }

    #[test]
    fn single_catalog() {
        let catalogs = vec![[1, 3, 5, 7, 8, 9]];
        let searcher = FCSearcher::new(&catalogs);

        for key in 0..=10 {
            let ans = catalogs[0].partition_point(|num| num < &key);
            assert_eq!(searcher.search(&key)[0], ans, "key: {key}");
        }
    }

    #[test]
    fn many_catalogs() {
        let catalogs = vec![vec![1, 3, 6, 10], vec![2, 4, 5, 7, 8, 9]];
        let searcher = FCSearcher::new(&catalogs);

        for key in 0..=11 {
            let ans1 = catalogs[0].partition_point(|num| num < &key);
            let ans2 = catalogs[1].partition_point(|num| num < &key);
            assert_eq!(
                searcher.search(&key),
                &[ans1, ans2],
                "key: {key}, searcher: {searcher:#?}"
            );
        }
    }
}
