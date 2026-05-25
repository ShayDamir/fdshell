#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::ffi::{CStr, CString};
use sys::Fd;

pub struct FdVars {
    map: HashMap<CString, Fd>,
}

impl FdVars {
    pub fn new() -> Self {
        FdVars {
            map: HashMap::new(),
        }
    }

    pub fn insert(&mut self, name: CString, fd: Fd) -> Option<Fd> {
        self.map.insert(name, fd)
    }

    pub fn resolve(&self, name: &CStr) -> Option<&Fd> {
        self.map.get(name)
    }

    pub fn remove(&mut self, name: &CStr) -> Option<Fd> {
        self.map.remove(name)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&CStr, i32)> {
        self.map.iter().map(|(k, v)| (k.as_c_str(), v.as_raw()))
    }
}
