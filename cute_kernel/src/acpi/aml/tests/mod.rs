use crate::{
    acpi::{SDTables, dtst::DSDT},
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
use super::{
    AmlError, data_objects::AmlString, executor::AmlVM, named_objects::NamePath, namenode::AmlData,
};

#[cfg(test)]
use alloc::boxed::Box;

#[cfg(test)]
fn aml_test_case(boxed_data: &[u8]) -> (AmlVM, AmlData) {
    let data = into_dstd(boxed_data).unwrap();
    let mut runner = data.create_amlvm();
    if let Err(e) = runner.exec() {
        panic!("{e}");
    }
    let path = NamePath::from_string("\\\\MAIN").unwrap();
    let args = [];
    match runner.get_or_exec(path, &args) {
        Ok(data) => (runner, data),
        Err(e @ AmlError::ScopeErr(_, _)) => panic!("panic_a : {}", e),
        Err(e) => panic!("panic_b : {}", e),
    }
}

///Test passed
#[test_case]
fn refrences_0() {
    let boxed = Box::new(*include_bytes!("./compiled/references-0.aml"));
    let (vm, res) = aml_test_case(boxed.as_slice());
    assert!(res.string(&vm).unwrap() == AmlString(Box::new(*b"MyString")));
}

///Test passed
#[test_case]
fn refrences_1() {
    let boxed = Box::new(*include_bytes!("./compiled/references-1.aml"));
    let (vm, res) = aml_test_case(boxed.as_slice());
    assert!(res.string(&vm).unwrap() == AmlString(Box::new(*b"MyString")));
}

///Test passed
#[test_case]
fn refrences_3() {
    let boxed = Box::new(*include_bytes!("./compiled/references-3.aml"));
    let (vm, res) = aml_test_case(boxed.as_slice());
    assert!(res.string(&vm).unwrap() == AmlString(Box::new(*b"WHY?")));
}

///Test passed
#[test_case]
fn while_break() {
    let boxed = Box::new(*include_bytes!("./compiled/while-break.aml"));
    let (vm, res) = aml_test_case(boxed.as_slice());
    assert!(res.interger(&vm).unwrap() == 10);
}

///Test passed
#[test_case]
fn while_break_deep() {
    let boxed = Box::new(*include_bytes!("./compiled/while-break-deep.aml"));
    let (vm, res) = aml_test_case(boxed.as_slice());
    assert!(res.interger(&vm).unwrap() == 1);
}

///Test passed
#[test_case]
fn while_continue() {
    let boxed = Box::new(*include_bytes!("./compiled/while-continue.aml"));
    let (vm, res) = aml_test_case(boxed.as_slice());
    assert!(res.interger(&vm).unwrap() == 10);
}

///Test passed
#[test_case]
fn while_continue_deep() {
    let boxed = Box::new(*include_bytes!("./compiled/while-continue-deep.aml"));
    let (vm, res) = aml_test_case(boxed.as_slice());
    assert!(res.interger(&vm).unwrap() == 3);
}

///Test passed
#[test_case]
fn scope() {
    let boxed = Box::new(*include_bytes!("./compiled/scope.aml"));
    let (vm, res) = aml_test_case(boxed.as_slice());
    assert!(res.interger(&vm).unwrap() == 13);
}

///Test passed
#[test_case]
fn method_calls() {
    let boxed = Box::new(*include_bytes!("./compiled/method-calls.aml"));
    let (vm, res) = aml_test_case(boxed.as_slice());
    assert!(res.interger(&vm).unwrap() == 8);
}
