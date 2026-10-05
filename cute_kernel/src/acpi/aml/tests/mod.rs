use crate::{
    acpi::{SDTables, aml::data_objects::AmlString, dtst::DSDT},
    memory::addresses::VirtAddr,
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
use super::{AmlError, named_objects::NamePath, namenode::AmlData};

use alloc::boxed;
#[cfg(test)]
use alloc::boxed::Box;

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
        Err(e @ AmlError::ScopeErr(_, _)) => panic!("panic_a : {}", e),
        Err(e) => panic!("panic_b : {}", e),
    }
}

///Test passed
#[test_case]
fn refrences_0() {
    let boxed = Box::new(*include_bytes!("./compiled/references-0.aml"));
    let res = aml_test_case(boxed.as_slice());
    match res {
        AmlData::String(string) => assert!(string == AmlString(Box::new(*b"MyString"))),
        d => panic!("Expected aml string found {:?}", d),
    }
}

///Test passed
#[test_case]
fn refrences_1() {
    let boxed = Box::new(*include_bytes!("./compiled/references-1.aml"));
    let res = aml_test_case(boxed.as_slice());
    match res {
        AmlData::String(string) => assert!(string == AmlString(Box::new(*b"MyString"))),
        d => panic!("Expected aml string found {:?}", d),
    }
}

///Test passed
#[test_case]
fn refrences_3() {
    let boxed = Box::new(*include_bytes!("./compiled/references-3.aml"));
    let res = aml_test_case(boxed.as_slice());
    match res {
        AmlData::String(string) => assert!(string == AmlString(Box::new(*b"WHY?"))),
        d => panic!("Expected aml string found {:?}", d),
    }
}

///Test passed
#[test_case]
fn while_break() {
    let boxed = Box::new(*include_bytes!("./compiled/while-break.aml"));
    assert!(aml_test_case(boxed.as_slice()).interger().unwrap() == 10);
}

///Test passed
#[test_case]
fn while_break_deep() {
    let boxed = Box::new(*include_bytes!("./compiled/while-break-deep.aml"));
    assert!(aml_test_case(boxed.as_slice()).interger().unwrap() == 1);
}

///Test passed
#[test_case]
fn while_continue() {
    let boxed = Box::new(*include_bytes!("./compiled/while-continue.aml"));
    assert!(aml_test_case(boxed.as_slice()).interger().unwrap() == 10);
}

///Test passed
#[test_case]
fn while_continue_deep() {
    let boxed = Box::new(*include_bytes!("./compiled/while-continue-deep.aml"));
    assert!(aml_test_case(boxed.as_slice()).interger().unwrap() == 3);
}

///Test passed
#[test_case]
fn scope() {
    let boxed = Box::new(*include_bytes!("./compiled/scope.aml"));
    assert!(aml_test_case(boxed.as_slice()).interger().unwrap() == 13);
}

///Test passed
#[test_case]
fn method_calls() {
    let boxed = Box::new(*include_bytes!("./compiled/method-calls.aml"));
    assert!(aml_test_case(boxed.as_slice()).interger().unwrap() == 8)
}
