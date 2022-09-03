#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

pub mod logger;
pub mod interrupts;
pub mod gdt;

use boot_info::BootInfo;

pub fn init(boot_info: &BootInfo) {
    gdt::init();
    interrupts::init_idt();
    unsafe { interrupts::PICS.lock().initialize(); }

    logger::init_logger(&boot_info.framebuffer);

    // 启用中断
    x86_64::instructions::interrupts::enable();
}

pub fn hlt_loop() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}