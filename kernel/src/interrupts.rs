// https://wiki.osdev.org/Exceptions

use lazy_static::lazy_static;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        idt.double_fault.set_handler_fn(double_fault_handler);
        
        idt
    };
}

extern "x86-interrupt" fn breakpoint_handler(
    stack_frame: InterruptStackFrame)
{
    log::debug!("BREAKPOINT\n{:#?}", stack_frame);
}


/// 只有先触发第一类异常时, 触发第二类异常才会触发 double falut
/// ```text
/// | ------------------------ | ------------------------ |
/// | First Exception          |   Second Exception       |
/// | ------------------------ | ------------------------ |
/// | Divide-by-zero,          | Segment Not Present,     |
/// | Invalid TSS,             | Stack-Segment Fault,     |
/// | Segment Not Present,     | General Protection Fault |
/// | Stack-Segment Fault,     | Invalid TSS,             |
/// | General Protection Fault |                          |
/// | ------------------------ | ------------------------ |
/// |                          | Page Fault,              |
/// |                          | Invalid TSS,             |
/// |        Page Fault        | Segment Not Present,     |
/// |                          | Stack-Segment Fault,     |
/// |                          | General Protection Fault |
/// | ------------------------ | ------------------------ |
/// ```
/// 
/// double fault 的 error code 恒为 0
extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame, _error_code: u64) -> !
{
    panic!("DOUBLE FAULT\n{:#?}", stack_frame);
}

pub fn init_idt() {
    IDT.load();
}