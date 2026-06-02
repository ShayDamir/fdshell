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

    pub fn resolve(&self, name: &ShortCStr) -> Option<&LocalFd> {
        self.map.get(name)
    }

    pub fn remove(&mut self, name: &ShortCStr) -> Option<LocalFd> {
        self.map.remove(name)
    }
}
