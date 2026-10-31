use crate::{
    acpi::{SDTables, dtst::DSDT},
    memory::addresses::VirtAddr,
};

fn into_dstd(data: &[u8]) -> Option<&DSDT> {
    let ptr = VirtAddr::from_ref(data);
    match SDTables::load(ptr).ok()? {
        SDTables::SSDT(dsdt) | SDTables::DSDT(dsdt) => Some(dsdt),
        _ => None,
    }
}

#[cfg(test)]
use super::{
    AmlError, data_objects::AmlString, executor::AmlVM, named_objects::NamePath, namenode::AmlData,
};

#[cfg(test)]
use core::cell::RefCell;

#[cfg(test)]
use alloc::{boxed::Box, rc::Rc};

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
        Err(AmlError::ScopeErr(name, stackframe)) => panic!("Error in  : {}\n{}", name, stackframe),
        Err(e) => panic!("panic_b : {}", e),
    }
}

///Test passed
#[test_case]
fn table_loading_0() {
    let boxed = Box::new(*include_bytes!("./compiled/table-loading-0.aml"));
    let (vm, res) = aml_test_case(boxed.as_slice());
    assert!(res.string(&vm).unwrap() == AmlString::from_bytes(b"Hello World 123").unwrap());
}

///Test passed
#[test_case]
fn ref_modify_via_method_call() {
    let boxed = Box::new(*include_bytes!("./compiled/ref-modify-via-method-call.aml"));
    let (vm, res) = aml_test_case(boxed.as_slice());
    assert!(res.get_interger(&vm).unwrap() == 124);
}

///Test passed
#[test_case]
fn ref_modify_indirect() {
    let boxed = Box::new(*include_bytes!("./compiled/ref-modify-indirect.aml"));
    let (vm, res) = aml_test_case(boxed.as_slice());
    assert!(res.get_interger(&vm).unwrap() == 0x124);
}

///Test passed
#[test_case]
fn return0_indirect() {
    let boxed = Box::new(*include_bytes!("./compiled/return0-indirect.aml"));
    let (vm, res) = aml_test_case(boxed.as_slice());
    assert!(res.get_interger(&vm).unwrap() == 0x0);
}

///Test passed
#[test_case]
fn return1_using_ifs() {
    let boxed = Box::new(*include_bytes!("./compiled/return1-using-ifs.aml"));
    let (vm, res) = aml_test_case(boxed.as_slice());
    assert!(res.get_interger(&vm).unwrap() == 0x1);
}

///Test passed
#[test_case]
fn return_0xdead_double_indirect() {
    let boxed = Box::new(*include_bytes!(
        "./compiled/return-0xdead-double-indirect.aml"
    ));
    let (vm, res) = aml_test_case(boxed.as_slice());
    assert!(res.get_interger(&vm).unwrap() == 0xDEAD);
}

///Test passed
#[test_case]
fn multi_level_ref() {
    let boxed = Box::new(*include_bytes!("./compiled/multilevel-ref.aml"));
    let (vm, res) = aml_test_case(boxed.as_slice());
    assert!(res.get_interger(&vm).unwrap() == 0xDEAD_0005);
}

///Test passed
#[test_case]
fn empty_objects() {
    let boxed = Box::new(*include_bytes!("./compiled/empty-objects.aml"));
    let (vm, res) = aml_test_case(boxed.as_slice());
    assert!(res.get_interger(&vm).unwrap() == 0);
}

///Test passsed
#[test_case]
fn empty_buffers() {
    let boxed = Box::new(*include_bytes!("./compiled/empty-buffers.aml"));
    let (vm, res) = aml_test_case(boxed.as_slice());
    assert!(res.get_interger(&vm).unwrap() == 0);
}

///Test passed
#[test_case]
fn copy_object_self() {
    let boxed = Box::new(*include_bytes!("./compiled/copy-object-self.aml"));
    let (vm, res) = aml_test_case(boxed.as_slice());
    assert!(res.string(&vm).unwrap() == AmlString::from_bytes(b"Hello World").unwrap());
}

///Test passed
#[test_case]
fn complex_package() {
    let boxed = Box::new(*include_bytes!("./compiled/complex-package.aml"));
    let (vm, res) = aml_test_case(boxed.as_slice());
    assert!(res.get_interger(&vm).unwrap() == 1);
}

///Test passed
#[test_case]
fn copy_a_method() {
    let boxed = Box::new(*include_bytes!("./compiled/copy-a-method.aml"));
    let (vm, res) = aml_test_case(boxed.as_slice());
    assert!(res.get_interger(&vm).unwrap() == 37_3592_8559);
}

///Test passed
#[test_case]
fn to_interger() {
    let boxed = Box::new(*include_bytes!("./compiled/to-interger.aml"));
    let (vm, res) = aml_test_case(boxed.as_slice());
    assert!(res.get_interger(&vm).unwrap() == 0);
}

///Test passed
#[test_case]
fn to_x() {
    let boxed = Box::new(*include_bytes!("./compiled/to-x.aml"));
    let (vm, res) = aml_test_case(boxed.as_slice());
    assert!(res.get_interger(&vm).unwrap() == 0);
}

///Test passed
#[test_case]
fn unresolved_paths() {
    let boxed = Box::new(*include_bytes!("./compiled/unresolved-paths.aml"));
    let (vm, res) = aml_test_case(boxed.as_slice());
    assert!(res.get_interger(&vm).unwrap() == 1);
}

///Test passed
#[test_case]
fn while_dec_inc() {
    let boxed = Box::new(*include_bytes!("./compiled/while-dec-inc.aml"));
    let (vm, res) = aml_test_case(boxed.as_slice());
    assert!(res.get_interger(&vm).unwrap() == 10);
}

///Test passed
#[test_case]
fn refrences_0() {
    let boxed = Box::new(*include_bytes!("./compiled/references-0.aml"));
    let (vm, res) = aml_test_case(boxed.as_slice());
    assert!(res.string(&vm).unwrap() == AmlString::from_bytes(b"MyString").unwrap());
}

///Test passed
#[test_case]
fn refrences_1() {
    let boxed = Box::new(*include_bytes!("./compiled/references-1.aml"));
    let (vm, res) = aml_test_case(boxed.as_slice());
    assert!(res.string(&vm).unwrap() == AmlString::from_bytes(b"MyString").unwrap());
}

///Test passed
#[test_case]
fn refrences_3() {
    let boxed = Box::new(*include_bytes!("./compiled/references-3.aml"));
    let (vm, res) = aml_test_case(boxed.as_slice());
    assert!(res.string(&vm).unwrap() == AmlString::from_bytes(b"WHY?").unwrap());
}

///Test passed
#[test_case]
fn refrences_4() {
    let boxed = Box::new(*include_bytes!("./compiled/references-4.aml"));
    let (vm, res) = aml_test_case(boxed.as_slice());
    assert!(res.get_interger(&vm).unwrap() == 0x123);
}

///Test passed
#[test_case]
fn refrences_5() {
    let boxed = Box::new(*include_bytes!("./compiled/references-5.aml"));
    let (vm, res) = aml_test_case(boxed.as_slice());
    assert!(res.string(&vm).unwrap() == AmlString::from_bytes(b"Modified").unwrap());
}

///Test passed
#[test_case]
fn refrences_6() {
    let boxed = Box::new(*include_bytes!("./compiled/references-6.aml"));
    let (vm, res) = aml_test_case(boxed.as_slice());
    assert!(res.get_interger(&vm).unwrap() == 0xDEAD_BEEF);
}

///Test passsed
#[test_case]
fn refrences_7() {
    let boxed = Box::new(*include_bytes!("./compiled/references-7.aml"));
    let (vm, res) = aml_test_case(boxed.as_slice());
    assert!(res.string(&vm).unwrap() == AmlString::from_bytes(b"MyString").unwrap());
}

///Test passed
#[test_case]
fn refrences_8() {
    let boxed = Box::new(*include_bytes!("./compiled/references-8.aml"));
    let (vm, res) = aml_test_case(boxed.as_slice());
    assert!(res.get_interger(&vm).unwrap() == 0xDEAD_BEEF);
}

///Test passed
#[test_case]
fn refrences_9() {
    let boxed = Box::new(*include_bytes!("./compiled/references-9.aml"));
    let (vm, res) = aml_test_case(boxed.as_slice());
    assert!(res.get_interger(&vm).unwrap() == 0x676E_6F6C_7972_6576);
}

#[test_case]
fn refrences_10() {
    let boxed = Box::new(*include_bytes!("./compiled/references-10.aml"));
    let (vm, res) = aml_test_case(boxed.as_slice());
    assert!(res.get_interger(&vm).unwrap() == 0x100A);
}

///Test passed
#[test_case]
fn while_break() {
    let boxed = Box::new(*include_bytes!("./compiled/while-break.aml"));
    let (vm, res) = aml_test_case(boxed.as_slice());
    assert!(res.get_interger(&vm).unwrap() == 10);
}

///Test passed
#[test_case]
fn while_break_deep() {
    let boxed = Box::new(*include_bytes!("./compiled/while-break-deep.aml"));
    let (vm, res) = aml_test_case(boxed.as_slice());
    assert!(res.get_interger(&vm).unwrap() == 1);
}

///Test passed
#[test_case]
fn while_continue() {
    let boxed = Box::new(*include_bytes!("./compiled/while-continue.aml"));
    let (vm, res) = aml_test_case(boxed.as_slice());
    assert!(res.get_interger(&vm).unwrap() == 10);
}

///Test passed
#[test_case]
fn while_continue_deep() {
    let boxed = Box::new(*include_bytes!("./compiled/while-continue-deep.aml"));
    let (vm, res) = aml_test_case(boxed.as_slice());
    assert!(res.get_interger(&vm).unwrap() == 3);
}

///Test passed
#[test_case]
fn scope() {
    let boxed = Box::new(*include_bytes!("./compiled/scope.aml"));
    let (vm, res) = aml_test_case(boxed.as_slice());
    assert!(res.get_interger(&vm).unwrap() == 13);
}

///Test passed
#[test_case]
fn method_calls() {
    let boxed = Box::new(*include_bytes!("./compiled/method-calls.aml"));
    let (vm, res) = aml_test_case(boxed.as_slice());
    assert!(res.get_interger(&vm).unwrap() == 8);
}
