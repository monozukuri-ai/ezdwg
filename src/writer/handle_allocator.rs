use std::collections::HashSet;

use crate::core::error::{DwgError, ErrorKind};
use crate::core::result::Result;

#[derive(Debug, Clone)]
pub struct HandleAllocator {
    next: u64,
    used: HashSet<u64>,
}

impl Default for HandleAllocator {
    fn default() -> Self {
        Self::new(1)
    }
}

impl HandleAllocator {
    pub fn new(start: u64) -> Self {
        Self {
            next: start.max(1),
            used: HashSet::new(),
        }
    }

    pub fn with_used(start: u64, used_handles: impl IntoIterator<Item = u64>) -> Self {
        let mut out = Self::new(start);
        for handle in used_handles {
            let _ = out.reserve(handle);
        }
        out
    }

    pub fn reserve(&mut self, handle: u64) -> Result<()> {
        if handle == 0 {
            return Err(DwgError::new(
                ErrorKind::Format,
                "handle 0 is reserved and cannot be allocated",
            ));
        }
        if !self.used.insert(handle) {
            return Err(DwgError::new(
                ErrorKind::Resolve,
                format!("duplicate handle reservation: {handle}"),
            ));
        }
        if handle == self.next {
            while self.used.contains(&self.next) {
                if self.next == u64::MAX {
                    return Err(DwgError::new(
                        ErrorKind::Unsupported,
                        "handle space exhausted",
                    ));
                }
                self.next += 1;
            }
        }
        Ok(())
    }

    pub fn allocate(&mut self) -> Result<u64> {
        while self.used.contains(&self.next) {
            if self.next == u64::MAX {
                return Err(DwgError::new(
                    ErrorKind::Unsupported,
                    "handle space exhausted",
                ));
            }
            self.next += 1;
        }
        let handle = self.next;
        self.used.insert(handle);
        if self.next == u64::MAX {
            self.next = u64::MAX;
        } else {
            self.next += 1;
        }
        Ok(handle)
    }

    pub fn is_reserved(&self, handle: u64) -> bool {
        self.used.contains(&handle)
    }
}

#[cfg(test)]
mod tests {
    use super::HandleAllocator;

    #[test]
    fn allocates_monotonic_handles() {
        let mut allocator = HandleAllocator::new(10);
        assert_eq!(allocator.allocate().unwrap(), 10);
        assert_eq!(allocator.allocate().unwrap(), 11);
        allocator.reserve(20).unwrap();
        assert_eq!(allocator.allocate().unwrap(), 12);
        assert!(allocator.is_reserved(20));
    }
}
