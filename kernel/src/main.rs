#![no_std]
#![no_main]

use kernel::gdt;
use kernel::logger;
use kernel::interrupts;

use core::{
    arch::asm,
    panic::PanicInfo,
};
use boot_info::BootInfo;



#[no_mangle]
pub extern "C" fn _start(boot_info: &'static mut BootInfo) -> ! {
    logger::init_logger(&boot_info.framebuffer);

    interrupts::init();
    gdt::init();

    // 触发断点
    x86_64::instructions::interrupts::int3();

    // 在设置了 IST 后, stack overflow 不会导致 triple fault
    stack_overflow();

    // 触发 double fault
    unsafe {
        *(0x114514 as *mut u64) = 24;
    };

    panic!("DEAK LOCK");
}

#[allow(unconditional_recursion)]
fn stack_overflow() {
    let mut rsp: u64;
    unsafe {
        asm!("mov {}, rsp", out(reg)rsp);
    }
    log::debug!("rsp = {:#x}", rsp);
    stack_overflow();
}


#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    unsafe {
        logger::LOGGER
            .get()
            .map(|l| l.force_unlock())
    };
    log::error!("{}", info);

    loop {
        unsafe { asm!("cli; hlt") };
    }
}
