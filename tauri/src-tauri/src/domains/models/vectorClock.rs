use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VectorClock {
    // Maps user_id to their logical clock value
    pub clock: HashMap<String, u64>,
}

impl VectorClock {
    pub fn new() -> Self {
        Self {
            clock: HashMap::new(),
        }
    }

    // Initialize with a single user's first update
    pub fn initialize(user_id: &str) -> Self {
        let mut clock = HashMap::new();
        clock.insert(user_id.to_string(), 1);
        Self { clock }
    }

    // Increment a user's counter
    pub fn increment(&mut self, user_id: &str) {
        let counter = self.clock.entry(user_id.to_string()).or_insert(0);
        *counter += 1;
    }

    // Merge with another vector clock (take maximum of each entry)
    pub fn merge(&mut self, other: &VectorClock) {
        for (user_id, &counter) in &other.clock {
            let entry = self.clock.entry(user_id.clone()).or_insert(0);
            *entry = std::cmp::max(*entry, counter);
        }
    }

    // Returns true if self happens-before other
    pub fn happens_before(&self, other: &VectorClock) -> bool {
        // At least one entry in other is greater than in self
        let has_greater = other.clock.iter().any(|(user_id, &counter)| {
            self.clock
                .get(user_id)
                .map_or(true, |&self_counter| self_counter < counter)
        });

        // No entry in self is greater than in other
        let no_greater = !self.clock.iter().any(|(user_id, &counter)| {
            other
                .clock
                .get(user_id)
                .map_or(true, |&other_counter| counter > other_counter)
        });

        has_greater && no_greater
    }

    // Returns true if other happens-before self
    pub fn happens_after(&self, other: &VectorClock) -> bool {
        other.happens_before(self)
    }

    // Returns true if self and other are concurrent
    pub fn is_concurrent_with(&self, other: &VectorClock) -> bool {
        !self.happens_before(other) && !other.happens_before(self)
    }
}
