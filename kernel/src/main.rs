#![no_std]
#![no_main]

extern crate alloc;

use kernel::task::executor::Executor;
pub use kernel::{print, println};

use kernel::memory;
use kernel::task::{Task, spawn};
use kernel::task::keyboard::print_keypresses;
use boot_info::BootInfo;
use x86_64::VirtAddr;

#[no_mangle]
pub extern "C" fn _start(boot_info: &'static mut BootInfo) -> ! {
    kernel::init(&boot_info);
    
    log::info!("Running in kernel");
    log::info!("{:#?}", boot_info);

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator = unsafe {
        memory::BootInfoFrameAllocator::init(&boot_info.memory_regions)
    };
    // allocate a number on the heap
    kernel::allocator::init_heap(&mut mapper, &mut frame_allocator)
        .expect("heap initialization failed");
    
    let mut executor = Executor::new();
    executor.spawn(Task::new(example_task()));
    executor.spawn(Task::new(print_keypresses()));
    
    executor.run();
}

async fn async_number() -> u32 {
    42
}

async fn example_task() {
    let number = async_number().await;
    println!("async number: {}", number);
    for i in 0..1000000000 {
        if i % 100000000 == 0 {
            spawn(global_spawn(i / 100000000));
        }
    }
}

async fn global_spawn(i: i32) {
    let arr = [i, i*i];
    let arr_ref = &arr;

    println!("{:p}", arr_ref);

    let num = async_number().await;

    println!("{:p}", arr_ref);
    
    println!("{}: Global spawn {} !", i, num);
}