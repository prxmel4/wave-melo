//! Pure queue-index helpers shared by queue management commands.

use std::collections::VecDeque;

use crate::error::BotError;

/// Convert a user-facing 1-based index into a validated 0-based index.
pub fn zero_based(index: usize, len: usize) -> Result<usize, BotError> {
    if index == 0 || index > len {
        return Err(BotError::InvalidQueueIndex(index.to_string()));
    }
    Ok(index - 1)
}

/// Validate both endpoints of a 1-based queue move.
pub fn move_indices(len: usize, from: usize, to: usize) -> Result<(usize, usize), BotError> {
    Ok((zero_based(from, len)?, zero_based(to, len)?))
}

/// Move an element between two validated 0-based indices.
pub fn reordered<T>(queue: VecDeque<T>, from: usize, to: usize) -> VecDeque<T> {
    let mut queue = queue;
    if from >= queue.len() || to >= queue.len() || from == to {
        return queue;
    }
    if let Some(item) = queue.remove(from) {
        queue.insert(to, item);
    }
    queue
}

/// Split a queue into a jump target and the remaining entries.
pub fn jump_plan<T>(queue: VecDeque<T>, one_based: usize) -> Result<(T, VecDeque<T>), BotError> {
    let index = zero_based(one_based, queue.len())?;
    let mut queue = queue;
    let target = queue
        .remove(index)
        .ok_or_else(|| BotError::InvalidQueueIndex(one_based.to_string()))?;
    Ok((target, queue))
}

#[cfg(test)]
mod tests {
    use super::{jump_plan, move_indices, reordered, zero_based};
    use crate::error::BotError;
    use std::collections::VecDeque;

    fn queue() -> VecDeque<u8> {
        VecDeque::from(vec![1, 2, 3, 4, 5])
    }

    #[test]
    fn validates_one_based_indices() {
        assert_eq!(zero_based(1, 5).unwrap(), 0);
        assert_eq!(zero_based(5, 5).unwrap(), 4);
        for invalid in [0, 6, usize::MAX] {
            assert!(matches!(
                zero_based(invalid, 5),
                Err(BotError::InvalidQueueIndex(_))
            ));
        }
    }

    #[test]
    fn validates_move_endpoints() {
        assert_eq!(move_indices(5, 5, 1).unwrap(), (4, 0));
        assert!(matches!(
            move_indices(5, 1, 6),
            Err(BotError::InvalidQueueIndex(_))
        ));
    }

    #[test]
    fn reorders_without_losing_entries() {
        assert_eq!(
            reordered(queue(), 0, 2),
            VecDeque::from(vec![2, 3, 1, 4, 5])
        );
        assert_eq!(
            reordered(queue(), 4, 0),
            VecDeque::from(vec![5, 1, 2, 3, 4])
        );
        assert_eq!(reordered(queue(), 2, 2), queue());
        assert_eq!(
            reordered(VecDeque::<u8>::new(), 0, 0),
            VecDeque::<u8>::new()
        );
    }

    #[test]
    fn jump_plan_extracts_the_target() {
        let (target, rest) = jump_plan(queue(), 3).unwrap();
        assert_eq!(target, 3);
        assert_eq!(rest, VecDeque::from(vec![1, 2, 4, 5]));
        assert!(matches!(
            jump_plan(queue(), 0),
            Err(BotError::InvalidQueueIndex(_))
        ));
        assert!(matches!(
            jump_plan(VecDeque::<u8>::new(), 1),
            Err(BotError::InvalidQueueIndex(_))
        ));
    }
}
