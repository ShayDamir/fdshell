#![forbid(unsafe_code)]

use std::collections::HashMap;
use sys::Fd;
use sys::ShortCStr;

pub struct FdVars {
    map: HashMap<ShortCStr, Fd>,
}

impl FdVars {
    pub fn new() -> Self {
        FdVars {
            map: HashMap::new(),
        }
    }

    pub fn insert(&mut self, name: ShortCStr, fd: Fd) -> Option<Fd> {
        self.map.insert(name, fd)
    }

    pub fn resolve(&self, name: &[u8]) -> Option<&Fd> {
        self.map.get(name)
    }

    pub fn remove(&mut self, name: &[u8]) -> Option<Fd> {
        self.map.remove(name)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&[u8], i32)> {
        self.map.iter().map(|(k, v)| (k.as_bytes(), v.as_raw()))
    }
}
