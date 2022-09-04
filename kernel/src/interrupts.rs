//! # Interrupt Descriptor Table (IDT)
//! 为了处理 CPU 异常, 需要建立 IDT。
//! 
//! ## IDT 的结构
//! ```text
//! 
//! Type      Name                                    Description
//!  u16  Function Pointer [0:15]     The lower bits of the pointer to the handler function.
//!  u16  GDT selector                Selector of a code segment in the global descriptor table.
//!  u16  Options                                     (see below)
//!  u16  Function Pointer [16:31]    The middle bits of the pointer to the handler function.
//!  u32  Function Pointer [32:63]    The remaining bits of the pointer to the handler function.
//!  u32  Reserved    
//! 
//! 
//!  Bits               Name                                 Description
//! 0 - 2    Interrupt Stack Table Index         0: Don’t switch stacks, 1-7: Switch to the n-th stack in the Interrupt Stack Table when this handler is called.
//! 3 - 7             Reserved      
//!   8      0: Interrupt Gate, 1: Trap Gate     If this bit is 0, interrupts are disabled when this handler is called.
//! 9 - 11           must be one      
//!   12             must be zero          
//! 13 ‑ 14    Descriptor Privilege Level (DPL)      The minimal privilege level required for calling this handler.
//!   15               Present      
//! ```
//! 
//! 每一种异常都有一个预先定义好的 IDT 下标 (也称: Vector nr), 参考: https://wiki.osdev.org/Exceptions 。
//! 
//! ## Gate Types
//! 参考: https://wiki.osdev.org/IDT#Gate_Types
//! 
//! 中断可以分为两类, 一类是在代码执行时由于错误代码产生的(如: 除零异常), 一类是为了处理
//! 和当前执行的代码无关的事件而产生的(如: 通过 INT 指令产生的中断) 。第一类也称为 Traps ,
//! 在进入处理程序前, 保存的是当前运行的指令的地址, 用于重试, 而第二类中断保存的是下一条指令的地址。
//! 
//! 而在处理 Traps 的时候可能会发生新的中断, 而发生第二类中断的时候, 中断会被屏蔽, 直到发出信号 
//! End of Interrupt (EOI) 。
//! 
//! 而 Trap Gate 对应的就是第一类中断 (Trap) , Interrupt Gate 对应的就是第二类中断。
//! 
//! # 异常发生时, CPU 会做什么?
//! 1. 保存上下文: 将一些寄存器的值(包括: RFLAGE)以及栈指针保存到栈上
//! 2. 读取相应的 IDT 项
//! 3. 检查相应的 IDT 项中是否提供了处理函数的入口, 如果没有就抛出 double fault
//! 4. 如果是 Interrupt Gate 就屏蔽中断
//! 5. 加载 IDT 项中指定的 GDT selector 到 CS 中
//! 6. 跳转到指定的处理函数
//! 
//! # CPU Exceptions
//! CPU 异常会在不同的情况下会被触发，如:
//! 
//! - 除以零
//! - 访问非法的虚拟地址
//! 
//! 具体见: https://wiki.osdev.org/Exceptions
//! 
//! # Hardware Interrupts
//! 中断也可以由硬件触发, 如: 时钟, 键盘等。而这些硬件中断信号的传输途径, 则如下图所示:
//! 
//! ```txt
//!                                     ____________             _____
//！               Timer ------------> |            |           |     |
//！               Keyboard ---------> | Interrupt  |---------> | CPU |
//！               Other Hardware ---> | Controller |           |_____|
//！               Etc. -------------> |____________|
//! ```
//! 
//! 硬件产生的中断信号通过 Interrupt Controller (中断控制器) 传输到 CPU 中。
//! 
//! ## The 8259 PIC
//! 
//! ```txt
//!                          ____________                          ____________
//!     Real Time Clock --> |            |   Timer -------------> |            |
//!     ACPI -------------> |            |   Keyboard-----------> |            |      _____
//!     Available --------> | Secondary  |----------------------> | Primary    |     |     |
//!     Available --------> | Interrupt  |   Serial Port 2 -----> | Interrupt  |---> | CPU |
//!     Mouse ------------> | Controller |   Serial Port 1 -----> | Controller |     |_____|
//!     Co-Processor -----> |            |   Parallel Port 2/3 -> |            |
//!     Primary ATA ------> |            |   Floppy disk -------> |            |
//!     Secondary ATA ----> |____________|   Parallel Port 1----> |____________|
//! 
//! ```
//! The 8259 has eight interrupt lines and several lines for communicating with the CPU. 
//! The typical systems back then were equipped with two instances of the 8259 PIC, 
//! one primary and one secondary PIC, connected to one of the interrupt lines of the primary
//! 
//! Each controller can be configured through two I/O ports, 
//! one “command” port and one “data” port. For the primary controller, 
//! these ports are 0x20 (command) and 0x21 (data). For the secondary controller, 
//! they are 0xa0 (command) and 0xa1 (data). 
//! For more information on how the PICs can be configured, see the article on https://wiki.osdev.org/8259_PIC .
//! 
//! # 调用约定 (Calling Convention)
//! 调用约定是程序在函数调用时传递参数和获取返回值的方式的约定, 如:
//! 
//! ```
//! extern "C"
//! ```
//! 
//! 而 x86 的中断函数在调用前需要进行保存上下文、还要考虑栈指针的对齐(因为一些 SSE 指令有对齐要求),
//! 错误码传递等问题。
//! 所以调用中断处理函数时不能像调用普通函数那样做, 需要使用一种特定的调用约定, rust 中为:
//! 
//! ```
//! extern "x86-interrupt"
//! ```
//! 
use crate::gdt;

use spin;
use pic8259::ChainedPics;
use lazy_static::lazy_static;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};

/// 预定义的 CPU Exception 已经占了 0 - 31 , 所以从 32 开始
pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

pub static PICS: spin::Mutex<ChainedPics> =
    spin::Mutex::new(unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) });

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = PIC_1_OFFSET,
    Keyboard,
}

impl InterruptIndex {
    fn as_u8(self) -> u8 {
        self as u8
    }

    fn as_usize(self) -> usize {
        usize::from(self.as_u8())
    }
}

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        idt.page_fault.set_handler_fn(page_fault_handler);
        idt.general_protection_fault.set_handler_fn(general_protection_handler);
        
        unsafe {
            idt.double_fault.set_handler_fn(double_fault_handler)
                .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);
        }

        idt[InterruptIndex::Timer.as_usize()].set_handler_fn(timer_interrupt_handler);
        idt[InterruptIndex::Keyboard.as_usize()].set_handler_fn(keyboard_interrupt_handler);

        idt
    };
}

extern "x86-interrupt" fn breakpoint_handler(
    stack_frame: InterruptStackFrame)
{
    log::debug!("BREAKPOINT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode
)
{
    use x86_64::registers::control::Cr2;
    // The CR2 register is automatically set by the CPU on a page fault 
    // and contains the accessed virtual address that caused the page fault. 
    log::error!(concat!(
        "EXCEPTION: PAGE FAULT\n",
        "Accessed Address: {:?}\n",
        "Error Code: {:?}\n",
        "{:#?}"
    ), Cr2::read(), error_code, stack_frame);
    
    crate::hlt_loop();
}

extern "x86-interrupt" fn general_protection_handler(
    stack_frame: InterruptStackFrame, error_code: u64)
{
    panic!("General protection\n{:#?}\nerror_code: {}", stack_frame, error_code);
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

extern "x86-interrupt" fn timer_interrupt_handler(
    _stack_frame: InterruptStackFrame)
{
    print!(".");

    // 发送 EOI 信号
    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Timer.as_u8());
    }
}

extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
    use pc_keyboard::{layouts, DecodedKey, HandleControl, Keyboard, ScancodeSet1};
    use spin::Mutex;
    use x86_64::instructions::port::Port;

    lazy_static! {
        static ref KEYBOARD: Mutex<Keyboard<layouts::Us104Key, ScancodeSet1>> = Mutex::new(
            // HandleControl 会将 ctrl + [a - z] 映射到 unicode 字符 'U+0001' - 'U+001a'
            // 但暂时不去处理这种情况, 所以选择 Ignore
            Keyboard::new(layouts::Us104Key, ScancodeSet1, HandleControl::Ignore)
        );
    }

    let mut keyboard = KEYBOARD.lock();

    // https://wiki.osdev.org/%228042%22_PS/2_Controller#Data_Port
    const PS2_CONTROLLER_DATA_PORT: u16 = 0x60;
    let mut port = Port::new(PS2_CONTROLLER_DATA_PORT);

    // 在读取 scancode 之前, 键盘不能继续输入
    let scancode: u8 = unsafe { port.read() };

    // scancode 是一个 u8 , 所以选择使用 add_byte
    if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
        if let Some(key) = keyboard.process_keyevent(key_event) {
            match key {
                DecodedKey::Unicode(character) => print!("{}", character),
                DecodedKey::RawKey(key) => print!("{:?}", key),
            }
        }
    }

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8());
    }
}

pub fn init_idt() {
    IDT.load();
}