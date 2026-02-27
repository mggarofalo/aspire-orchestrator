use std::collections::{HashMap, HashSet};
use std::net::TcpListener;
use std::sync::Mutex;

use crate::error::{OrchestratorError, Result};
use crate::models::PortAllocation;

pub struct PortAllocator {
    allocated: Mutex<HashSet<u16>>,
}

impl PortAllocator {
    pub fn new() -> Self {
        Self {
            allocated: Mutex::new(HashSet::new()),
        }
    }

    pub fn allocate(&self, name: &str) -> Result<PortAllocation> {
        let mut allocated = self.allocated.lock().unwrap();
        for _ in 0..100 {
            let port = find_available_port()?;
            if allocated.insert(port) {
                return Ok(PortAllocation {
                    name: name.to_string(),
                    port,
                });
            }
        }
        Err(OrchestratorError::PortAllocation(
            "could not find an available port after 100 attempts".into(),
        ))
    }

    pub fn allocate_for_overrides(
        &self,
        overrides: &HashMap<String, u16>,
    ) -> Result<Vec<PortAllocation>> {
        let mut allocations = Vec::new();
        for (name, &port) in overrides {
            let mut allocated = self.allocated.lock().unwrap();
            if allocated.contains(&port) {
                // Port already taken, allocate a random one instead
                drop(allocated);
                allocations.push(self.allocate(name)?);
            } else {
                allocated.insert(port);
                allocations.push(PortAllocation {
                    name: name.clone(),
                    port,
                });
            }
        }
        Ok(allocations)
    }

    pub fn release(&self, port: u16) {
        let mut allocated = self.allocated.lock().unwrap();
        allocated.remove(&port);
    }
}

impl Default for PortAllocator {
    fn default() -> Self {
        Self::new()
    }
}

fn find_available_port() -> Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|e| OrchestratorError::PortAllocation(e.to_string()))?;
    let port = listener
        .local_addr()
        .map_err(|e| OrchestratorError::PortAllocation(e.to_string()))?
        .port();
    Ok(port)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocate_returns_valid_port() {
        let allocator = PortAllocator::new();
        let alloc = allocator.allocate("TEST_PORT").unwrap();
        assert_eq!(alloc.name, "TEST_PORT");
        assert!(alloc.port > 0);
    }

    #[test]
    fn allocate_returns_unique_ports() {
        let allocator = PortAllocator::new();
        let a = allocator.allocate("PORT_A").unwrap();
        let b = allocator.allocate("PORT_B").unwrap();
        assert_ne!(a.port, b.port);
    }

    #[test]
    fn release_frees_port() {
        let allocator = PortAllocator::new();
        let alloc = allocator.allocate("TEST_PORT").unwrap();
        let port = alloc.port;
        allocator.release(port);
        let allocated = allocator.allocated.lock().unwrap();
        assert!(!allocated.contains(&port));
    }
}
