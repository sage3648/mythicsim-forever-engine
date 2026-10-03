//! Stable event ordering: earliest time, highest priority, then insertion order.
//! Payloads belong to the caller; the scheduler has no class or spell knowledge.

use std::{cmp::Ordering, collections::BinaryHeap};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Event<T> {
    pub(crate) time: u64,
    priority: i8,
    sequence: u64,
    pub(crate) kind: T,
}

impl<T> PartialEq for Event<T> {
    fn eq(&self, other: &Self) -> bool {
        (self.time, self.priority, self.sequence) == (other.time, other.priority, other.sequence)
    }
}
impl<T> Eq for Event<T> {}
impl<T> PartialOrd for Event<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl<T> Ord for Event<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .time
            .cmp(&self.time)
            .then(self.priority.cmp(&other.priority))
            .then(other.sequence.cmp(&self.sequence))
    }
}

pub(crate) fn enqueue<T>(
    queue: &mut BinaryHeap<Event<T>>,
    sequence: &mut u64,
    time: u64,
    priority: i8,
    kind: T,
) {
    queue.push(Event {
        time,
        priority,
        sequence: *sequence,
        kind,
    });
    *sequence += 1;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_timestamps_preserve_priority_and_insertion_order() {
        let mut queue = BinaryHeap::new();
        let mut sequence = 0;
        enqueue(&mut queue, &mut sequence, 10, -1, "impact");
        enqueue(&mut queue, &mut sequence, 10, 0, "first_cast");
        enqueue(&mut queue, &mut sequence, 10, 1, "mana_tick");
        enqueue(&mut queue, &mut sequence, 10, 0, "second_cast");
        enqueue(&mut queue, &mut sequence, 20, 100, "later");
        let mut order = Vec::new();
        while let Some(event) = queue.pop() {
            order.push(event.kind);
        }
        assert_eq!(
            order,
            ["mana_tick", "first_cast", "second_cast", "impact", "later"]
        );
    }
}
