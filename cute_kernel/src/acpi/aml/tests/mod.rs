use alloc::boxed::Box;

use crate::{
    acpi::{
        SDTables,
        aml::{
            AmlError,
            named_objects::{NamePath, NameSeg},
            namenode::AmlData,
        },
        dtst::DSDT,
    },
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

#[cfg(test)]
fn aml_test_case(boxed_data: &[u8]) -> AmlData {
    let data = into_dstd(boxed_data).unwrap();
    let mut runner = data.create_amlvm();
    if let Err(e) = runner.exec() {
        panic!("{e}");
    }
    let path = NamePath::from_string("\\\\MAIN").unwrap();
    let args = [];
    match runner.get_or_exec(path, &args) {
        Ok(data) => data,
        Err(e @ AmlError::ScopeErr(_, _)) => panic!("{}", e),
        Err(e) => panic!("{}", e),
    }
}

#[test_case]
fn while_break() {
    let boxed = Box::new(*include_bytes!("./compiled/while_break.aml"));
    assert!(aml_test_case(boxed.as_slice()).interger().unwrap() == 10);
}

#[test_case]
fn scope() {
    let boxed = Box::new(*include_bytes!("./compiled/scope.aml"));
    assert!(aml_test_case(boxed.as_slice()).interger().unwrap() == 13);
}
