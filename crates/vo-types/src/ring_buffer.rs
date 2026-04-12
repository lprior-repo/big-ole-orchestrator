use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RingBuffer<T, const CAPACITY: usize> {
    buffer: [Option<T>; CAPACITY],
    read_index: usize,
    write_index: usize,
    count: usize,
}

impl<T, const CAPACITY: usize> RingBuffer<T, CAPACITY>
where
    T: Clone + Default,
{
    pub fn new() -> Self {
        assert!(CAPACITY > 0, "RingBuffer capacity must be greater than 0");
        Self {
            buffer: std::array::from_fn(|_| None),
            read_index: 0,
            write_index: 0,
            count: 0,
        }
    }

    #[must_use]
    pub fn capacity(&self) -> usize {
        CAPACITY
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.count
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    #[must_use]
    pub fn is_full(&self) -> bool {
        self.count == CAPACITY
    }

    pub fn push(&mut self, item: T) -> Result<(), RingBufferError<T>> {
        if self.is_full() {
            return Err(RingBufferError::Full(item));
        }
        self.buffer[self.write_index] = Some(item);
        self.write_index = (self.write_index + 1) % CAPACITY;
        self.count += 1;
        Ok(())
    }

    pub fn pop(&mut self) -> Result<T, RingBufferError<T>> {
        if self.is_empty() {
            return Err(RingBufferError::Empty);
        }
        let item = self.buffer[self.read_index]
            .take()
            .ok_or_else(|| panic!("RingBuffer invariant violated: read_index points to None"))?;
        self.read_index = (self.read_index + 1) % CAPACITY;
        self.count -= 1;
        Ok(item)
    }

    #[must_use]
    pub fn peek(&self) -> Option<&T> {
        if self.is_empty() {
            return None;
        }
        self.buffer[self.read_index].as_ref()
    }

    pub fn clear(&mut self) {
        while !self.is_empty() {
            let _ = self.pop();
        }
    }

    pub fn try_push(&mut self, item: T) -> bool {
        self.push(item).is_ok()
    }

    pub fn try_pop(&mut self) -> Option<T> {
        self.pop().ok()
    }
}

impl<T, const CAPACITY: usize> Default for RingBuffer<T, CAPACITY>
where
    T: Clone + Default,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T, const CAPACITY: usize> fmt::Display for RingBuffer<T, CAPACITY>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "RingBuffer {{ len: {}/{}, read: {}, write: {} }}",
            self.count, CAPACITY, self.read_index, self.write_index
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RingBufferError<T> {
    #[error("ring buffer is full")]
    Full(T),
    #[error("ring buffer is empty")]
    Empty,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_buffer_new_creates_empty_buffer() {
        let rb: RingBuffer<i32, 4> = RingBuffer::new();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
        assert_eq!(rb.capacity(), 4);
        assert!(!rb.is_full());
    }

    #[test]
    fn ring_buffer_push_increments_count() {
        let mut rb: RingBuffer<i32, 4> = RingBuffer::new();
        assert!(rb.push(1).is_ok());
        assert_eq!(rb.len(), 1);
        assert!(!rb.is_empty());
    }

    #[test]
    fn ring_buffer_push_full_returns_error() {
        let mut rb: RingBuffer<i32, 2> = RingBuffer::new();
        assert!(rb.push(1).is_ok());
        assert!(rb.push(2).is_ok());
        let result = rb.push(3);
        assert!(result.is_err());
        if let Err(RingBufferError::Full(item)) = result {
            assert_eq!(item, 3);
        } else {
            panic!("Expected Full error");
        }
    }

    #[test]
    fn ring_buffer_pop_returns_items_in_fifo_order() {
        let mut rb: RingBuffer<i32, 4> = RingBuffer::new();
        rb.push(1).unwrap();
        rb.push(2).unwrap();
        rb.push(3).unwrap();
        assert_eq!(rb.pop().unwrap(), 1);
        assert_eq!(rb.pop().unwrap(), 2);
        assert_eq!(rb.pop().unwrap(), 3);
        assert!(rb.is_empty());
    }

    #[test]
    fn ring_buffer_pop_empty_returns_error() {
        let mut rb: RingBuffer<i32, 4> = RingBuffer::new();
        let result = rb.pop();
        assert!(result.is_err());
        assert!(matches!(result, Err(RingBufferError::Empty)));
    }

    #[test]
    fn ring_buffer_peek_returns_front_without_removing() {
        let mut rb: RingBuffer<i32, 4> = RingBuffer::new();
        rb.push(10).unwrap();
        rb.push(20).unwrap();
        assert_eq!(rb.peek(), Some(&10));
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.pop().unwrap(), 10);
        assert_eq!(rb.peek(), Some(&20));
    }

    #[test]
    fn ring_buffer_peek_empty_returns_none() {
        let rb: RingBuffer<i32, 4> = RingBuffer::new();
        assert_eq!(rb.peek(), None);
    }

    #[test]
    fn ring_buffer_wraps_around_correctly() {
        let mut rb: RingBuffer<i32, 3> = RingBuffer::new();
        rb.push(1).unwrap();
        rb.push(2).unwrap();
        rb.push(3).unwrap();
        assert!(rb.is_full());
        assert_eq!(rb.pop().unwrap(), 1);
        assert!(!rb.is_full());
        rb.push(4).unwrap();
        assert!(rb.is_full());
        assert_eq!(rb.pop().unwrap(), 2);
        assert_eq!(rb.pop().unwrap(), 3);
        assert_eq!(rb.pop().unwrap(), 4);
        assert!(rb.is_empty());
    }

    #[test]
    fn ring_buffer_clear_removes_all_items() {
        let mut rb: RingBuffer<i32, 4> = RingBuffer::new();
        rb.push(1).unwrap();
        rb.push(2).unwrap();
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn ring_buffer_try_push_returns_false_when_full() {
        let mut rb: RingBuffer<i32, 2> = RingBuffer::new();
        assert!(rb.try_push(1));
        assert!(rb.try_push(2));
        assert!(!rb.try_push(3));
    }

    #[test]
    fn ring_buffer_try_pop_returns_none_when_empty() {
        let mut rb: RingBuffer<i32, 4> = RingBuffer::new();
        assert_eq!(rb.try_pop(), None);
        rb.push(42).unwrap();
        assert_eq!(rb.try_pop(), Some(42));
        assert_eq!(rb.try_pop(), None);
    }

    #[test]
    fn ring_buffer_display_shows_state() {
        let mut rb: RingBuffer<i32, 5> = RingBuffer::new();
        rb.push(1).unwrap();
        rb.push(2).unwrap();
        let display = format!("{}", rb);
        assert!(display.contains("len: 2/5"));
    }

    #[test]
    #[should_panic(expected = "capacity must be greater than 0")]
    fn ring_buffer_zero_capacity_panics() {
        let _rb: RingBuffer<i32, 0> = RingBuffer::new();
    }

    #[test]
    fn ring_buffer_with_large_capacity() {
        let mut rb: RingBuffer<u64, 1024> = RingBuffer::new();
        for i in 0..100 {
            rb.push(i).unwrap();
        }
        assert_eq!(rb.len(), 100);
        assert!(!rb.is_full());
        for i in 0..100 {
            assert_eq!(rb.pop().unwrap(), i);
        }
        assert!(rb.is_empty());
    }

    #[test]
    fn ring_buffer_debug_display() {
        let rb: RingBuffer<i32, 4> = RingBuffer::new();
        let debug = format!("{:?}", rb);
        assert!(debug.contains("RingBuffer"));
    }
}
