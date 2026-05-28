#![forbid(unsafe_code)]

use std::collections::HashMap;
use sys::LocalFd;
use sys::ShortCStr;

pub struct FdVars {
    map: HashMap<ShortCStr, LocalFd>,
}

impl FdVars {
    pub fn new() -> Self {
        FdVars {
            map: HashMap::new(),
        }
    }

    pub fn insert(&mut self, name: ShortCStr, fd: LocalFd) -> Option<LocalFd> {
        self.map.insert(name, fd)
    }

    pub fn resolve(&self, name: &[u8]) -> Option<&LocalFd> {
        self.map.get(name)
    }

    pub fn remove(&mut self, name: &[u8]) -> Option<LocalFd> {
        self.map.remove(name)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&[u8], i32)> {
        self.map.iter().map(|(k, v)| (k.as_bytes(), v.as_raw()))
    }
}
