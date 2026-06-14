#![allow(clippy::unwrap_used)]

#[allow(unused_imports)]
use super::ForkCell;

#[test]
fn fork_cell_new() {
    let cell: ForkCell<i32> = ForkCell::new(42);
    assert_eq!(*cell.borrow().unwrap(), 42);
}

#[test]
fn borrow_fails_on_mut_borrow() {
    let cell = ForkCell::new(10);
    let _mut_ref = cell.borrow_mut();
    // Shared borrow should fail with EINVAL
    assert!(cell.borrow().is_err());
}

#[test]
fn borrow_mut_fails_on_shared_borrow() {
    let cell = ForkCell::new(10);
    let _ref = cell.borrow();
    // Exclusive borrow should fail with EINVAL
    assert!(cell.borrow_mut().is_err());
}

#[test]
fn multiple_shares_succeed() {
    let cell = ForkCell::new(7);
    let r1 = cell.borrow().unwrap();
    let r2 = cell.borrow().unwrap();
    assert_eq!(*r1, 7);
    assert_eq!(*r2, 7);
    drop(r1);
    // One shared borrow still active, but another should also work
    let r3 = cell.borrow().unwrap();
    assert_eq!(*r3, 7);
}

#[test]
fn borrow_mut_exclusive() {
    let cell = ForkCell::new(99);
    {
        let mut m = cell.borrow_mut().unwrap();
        *m += 1;
    }
    assert_eq!(*cell.borrow().unwrap(), 100);
}

#[test]
fn get_mut_compiles_with_refutable() {
    // get_mut requires &mut ForkCell<T>, which provides compile-time exclusivity.
    let mut cell = ForkCell::new(3);
    *cell.get_mut() += 7;
    assert_eq!(*cell.borrow().unwrap(), 10);
}

#[test]
fn borrow_count_increments_on_borrow() {
    let cell: ForkCell<u8> = ForkCell::new(0);
    assert!(cell.borrow_count() == 0);
    let _guard = cell.borrow().unwrap();
    assert!(cell.borrow_count() > 0);
}

#[test]
fn borrow_count_decrements_on_drop() {
    let cell: ForkCell<u8> = ForkCell::new(0);
    {
        let _guard = cell.borrow().unwrap();
        assert!(cell.borrow_count() > 0);
    }
    // After drop, count should be back to 0.
    assert!(cell.borrow_count() == 0);
}

#[test]
fn refmut_count_decrements_on_drop() {
    let cell: ForkCell<u8> = ForkCell::new(0);
    {
        let _guard = cell.borrow_mut().unwrap();
        assert!(cell.borrow_count() < 0);
    }
    // After drop, count should be back to 0.
    assert!(cell.borrow_count() == 0);
}
