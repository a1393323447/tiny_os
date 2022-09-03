//! # Global Descriptor Table (GDT)
//! 参考: https://wiki.osdev.org/GDT && https://wiki.osdev.org/GDT_Tutorial
//! 
//! 总得来说, 虽然 long mode 下, GDT 中的 limit 和 base 都不产生作用了, 但其它, 如:
//! 
//! - 特权级
//! - R/W
//! 
//! 依旧产生影响, 所以在 long mode 下依旧需要指定。
//! 
//! # Interrupt Stack Table (IST)
//! 
//! ## IST 是什么?
//! IST 中存放的是 7 个预定义好的、确保能用的栈空间的地址。
//! 
//! ## 为什么需要 IST ?
//! 因为在触发中断, 切换栈空间时, 可能会被切换到一个无效的栈空间中。
//! 这可能会导致触发 Triple Falut , 导致 rebot , 陷入死循环。
//! 
//! 例如：
//! 
//! 在触发栈溢出时, 会导致 Page Falut 。这时硬件会尝试调用 Page Falut 对应的
//! 中断处理函数。而这个过程中会产生参数的传递，也就会产生压栈，而这时栈是坏的。
//! 就会触发第二个 Page Falut ，产生 Double Falut 。类似地也就会产生 Triple 
//! Fault , 导致机器重启、循环。
//! 
//! ## 怎么样设置 IST ?
//! 要设置 IST , 就要对 Task State Segment (TSS) 有所了解。
//! 
//! ### 关于 TSS
//! 
//! 参考资料: Intel® 64 and IA-32 Architectures Software Developer’s Manual
//!           Volume 3 (3A, 3B, 3C & 3D): System Programming Guide 
//!           7.2.3 TSS Descriptor in 64-bit mode
//! 
//! The TSS, like all other segments, is defined by a segment descriptor.
//! TSS descriptors may only be placed in the GDT; they cannot be placed in an LDT or the IDT.
//! In 64-bit mode, task switching is not supported, but TSS descriptors still exist.
//! 
//! 在 32 位模式下, TSS 是一个保存了上下文信息的结构, 可以用于硬件层面上下文切换。
//! 但 64 位模式不再支持硬件层面的上下文切换。所以 TSS 的结构和相应的含义发生了改变：
//! 
//!  ```text
//!         Field              Type
//!       (reserved)            u32
//!  Privilege Stack Table   [u64; 3]
//!       (reserved)            u64
//!  Interrupt Stack Table   [u64; 7]
//!       (reserved)            u64
//!       (reserved)            u16
//!  I/O Map Base Address       u16
//! ```
//! 
//! 根据上表, IST 是 TSS 的一部分。
//! 
//! ### 如何设置 TSS
//! 由于历史遗留问题, 需要在 GDT 中创建一个 selector 指向 TSS ,
//! 然后再用 ltr 指令将这个 selector 加载到 task registor 中。
//! 这样就可以使用 TSS 。
//!  


use x86_64::VirtAddr;
use x86_64::structures::gdt::{GlobalDescriptorTable, SegmentSelector, Descriptor};
use x86_64::structures::tss::TaskStateSegment;
use lazy_static::lazy_static;

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

lazy_static! {
    static ref TSS: TaskStateSegment = {
        let mut tss = TaskStateSegment::new();
        tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
            const STACK_SIZE: usize = 4096 * 5;
            // 创建一个静态的栈
            static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

            let stack_start = VirtAddr::from_ptr(unsafe { &STACK });
            let stack_end = stack_start + STACK_SIZE;

            stack_end
        };

        tss
    };
}

lazy_static! {
    static ref GDT: (GlobalDescriptorTable, Selectors) = {
        let mut gdt = GlobalDescriptorTable::new();
        let code_selector = gdt.add_entry(Descriptor::kernel_code_segment());
        let data_selector = gdt.add_entry(Descriptor::kernel_data_segment());
        let tss_selector = gdt.add_entry(Descriptor::tss_segment(&TSS));

        (gdt, Selectors { code_selector, data_selector, tss_selector })
    };
}

/// 64 位段选择子
struct Selectors {
    code_selector: SegmentSelector,
    data_selector: SegmentSelector,
    tss_selector: SegmentSelector,
}

pub fn init() {
    use x86_64::instructions::tables::load_tss;
    use x86_64::instructions::segmentation::{CS, DS, ES, GS, FS, Segment};
    
    GDT.0.load();
    unsafe {
        CS::set_reg(GDT.1.code_selector);
        DS::set_reg(GDT.1.data_selector);
        ES::set_reg(GDT.1.data_selector);
        GS::set_reg(GDT.1.data_selector);
        FS::set_reg(GDT.1.data_selector);
        load_tss(GDT.1.tss_selector);
    }
}
