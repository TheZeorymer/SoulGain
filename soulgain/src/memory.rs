use std::collections::HashMap;

/// Precision for floating point addresses (1e10).
const PRECISION_SCALE: f64 = 10_000_000_000.0;

#[derive(Debug, Clone)]
pub struct MemorySystem {
    storage: HashMap<i64, f64>,
}

impl MemorySystem {
    pub fn new() -> Self {
        Self {
            storage: HashMap::new(),
        }
    }

    #[inline]
    fn quantize(addr: f64) -> Option<i64> {
        if !addr.is_finite() {
            return None;
        }
        Some((addr * PRECISION_SCALE).round() as i64)
    }

    pub fn read(&self, addr: f64) -> Option<f64> {
        let key = Self::quantize(addr)?;
        self.storage.get(&key).copied()
    }

    pub fn write(&mut self, addr: f64, val: f64) -> bool {
        let key = match Self::quantize(addr) {
            Some(k) => k,
            None => return false,
        };
        self.storage.insert(key, val);
        true
    }
}
