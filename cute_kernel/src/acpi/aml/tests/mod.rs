use alloc::boxed::Box;

use crate::{
    acpi::{SDTables, dtst::DSDT},
    memory::addresses::VirtAddr,
    serial_println,
};

fn into_dstd(data: &[u8]) -> Option<&DSDT> {
    let ptr = VirtAddr::from_ref(data);
    let data = SDTables::load(ptr).ok()?;
    if let SDTables::DSDT(dtst) = data {
        Some(dtst)
    } else {
        None
    }
}

#[test_case]
fn basic_test() {
    let boxed = Box::new(*include_bytes!("./compiled/basic.aml"));
    let data = into_dstd(boxed.as_slice()).unwrap();
    let mut runner = data.create_amlvm();
    if let Err(e) = runner.exec() {
        panic!("{e}");
    }
}

#[test_case]
fn basic_rtc_test() {
    let boxed = Box::new(*include_bytes!("./compiled/test_rtc.aml"));
    let data = into_dstd(boxed.as_slice()).unwrap();
    let mut runner = data.create_amlvm();
    if let Err(e) = runner.exec() {
        panic!("{e}");
    }
}
