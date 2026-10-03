//! Go `pendingActions` ordering with cancellable handles.
//!
//! Go keeps a sorted slice: earliest time first, then higher priority, then the action
//! inserted first. Cancelling removes an action without disturbing the others. A binary
//! heap with insertion sequence numbers and cancelled slots gives the same pop order.

use std::{cmp::Ordering, collections::BinaryHeap};

/// Identifies one scheduled action. A handle is never reused within a queue's lifetime.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Handle(u64);

#[derive(Debug)]
struct Entry {
    time: i64,
    priority: i32,
    sequence: u64,
}

impl PartialEq for Entry {
    fn eq(&self, other: &Self) -> bool {
        self.sequence == other.sequence
    }
}
impl Eq for Entry {}
impl PartialOrd for Entry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Entry {
    // BinaryHeap pops the greatest entry: earliest time, highest priority, first inserted.
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .time
            .cmp(&self.time)
            .then(self.priority.cmp(&other.priority))
            .then(other.sequence.cmp(&self.sequence))
    }
}

pub(crate) struct PendingQueue<T> {
    heap: BinaryHeap<Entry>,
    /// Payloads by sequence offset; `None` once popped or cancelled.
    slots: Vec<Option<T>>,
    base: u64,
    next: u64,
}

impl<T> Default for PendingQueue<T> {
    fn default() -> Self {
        Self {
            heap: BinaryHeap::new(),
            slots: Vec::new(),
            base: 0,
            next: 0,
        }
    }
}

impl<T> PendingQueue<T> {
    /// Remove every action. Handles from before the clear stay invalid.
    pub(crate) fn clear(&mut self) {
        self.heap.clear();
        self.slots.clear();
        self.base = self.next;
    }

    pub(crate) fn push(&mut self, time: i64, priority: i32, payload: T) -> Handle {
        let sequence = self.next;
        self.next += 1;
        self.heap.push(Entry {
            time,
            priority,
            sequence,
        });
        self.slots.push(Some(payload));
        Handle(sequence)
    }

    fn slot(&mut self, handle: Handle) -> Option<&mut Option<T>> {
        let index = handle.0.checked_sub(self.base)?;
        self.slots.get_mut(usize::try_from(index).ok()?)
    }

    /// Cancel a pending action. Returns false if it already ran or was cancelled.
    pub(crate) fn cancel(&mut self, handle: Handle) -> bool {
        self.slot(handle).and_then(Option::take).is_some()
    }

    /// The time of the next live action, dropping cancelled entries on the way.
    pub(crate) fn peek_time(&mut self) -> Option<i64> {
        while let Some(entry) = self.heap.peek() {
            let index = (entry.sequence - self.base) as usize;
            if self.slots[index].is_some() {
                return Some(entry.time);
            }
            self.heap.pop();
        }
        None
    }

    /// Pop the next live action.
    pub(crate) fn pop(&mut self) -> Option<(i64, Handle, T)> {
        while let Some(entry) = self.heap.pop() {
            let index = (entry.sequence - self.base) as usize;
            if let Some(payload) = self.slots[index].take() {
                return Some((entry.time, Handle(entry.sequence), payload));
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pops_by_time_then_priority_then_insertion_like_go() {
        let mut queue = PendingQueue::default();
        queue.push(10, 0, "first_gcd");
        queue.push(10, 1, "regen");
        let cancelled = queue.push(10, 3, "cancelled_dot");
        queue.push(10, 0, "second_gcd");
        queue.push(5, -1, "earlier");
        assert!(queue.cancel(cancelled));
        assert!(!queue.cancel(cancelled));
        let order: Vec<_> = std::iter::from_fn(|| queue.pop().map(|(_, _, p)| p)).collect();
        assert_eq!(order, ["earlier", "regen", "first_gcd", "second_gcd"]);
    }

    #[test]
    fn handles_from_before_a_clear_are_inert() {
        let mut queue = PendingQueue::default();
        let old = queue.push(1, 0, 1);
        queue.clear();
        let new = queue.push(1, 0, 2);
        assert!(!queue.cancel(old));
        assert_eq!(queue.pop().map(|(_, _, p)| p), Some(2));
        assert!(!queue.cancel(new));
    }
}
