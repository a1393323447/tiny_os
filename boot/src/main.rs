#![no_std]
#![no_main]
#![feature(step_trait)]
#![feature(maybe_uninit_slice)]

mod gdt;
mod memory;
mod logger;
mod loader;
mod utility;

use crate::logger::{VGA_BUFFER_START_ADDR, VGA_BUFFER_SIZE, VGA_TEXT_MODE_HEIGHT, VGA_TEXT_MODE_WIDTH};

use boot_info::FrameBufferInfo;

use utility::{SystemInfo, load_and_switch_to_kernel};
use memory::legacy_memory_region::LegacyFrameAllocator;

use core::{
    arch::{asm, global_asm},
    panic::PanicInfo,
};

use x86_64::{ PhysAddr, VirtAddr };
use x86_64::structures::paging::{
    FrameAllocator, OffsetPageTable, Mapper, PageTable, 
    PageTableFlags, PhysFrame, Size2MiB, Size4KiB,
};

global_asm!(include_str!("asm/stage_1.s"));
global_asm!(include_str!("asm/stage_2_real.s"));
global_asm!(include_str!("asm/stage_2_protected.s"));
global_asm!(include_str!("asm/stage_3.s"));

// Symbols defined in `linker.ld`
extern "C" {
    static mmap_ent: usize;
    static _memory_map: usize;
    static _kernel_size: usize;
    static _p4: usize;
    static _p3: usize;
}

#[no_mangle]
pub unsafe extern "C" fn stage_4() -> ! {
    // 设置栈段
    asm!(
        "mov ax, 0x0; mov ss, ax",
        out("ax") _,
    );

    {
        let logger = logger::LOGGER.get_or_init(move || logger::LockedLogger::new());
        log::set_logger(logger).expect("logger already set");
        log::set_max_level(log::LevelFilter::max());
    }

    let kernel_start = 0x400000;
    let kernel_size = &_kernel_size as *const _ as u64;
    let memory_map_addr = &_memory_map as *const _ as u64;
    let memory_map_entry_count = (mmap_ent & 0xff) as u64;

    log::info!("Booting(4)");

    bootloader_main(
        PhysAddr::new(kernel_start), 
        kernel_size, 
        VirtAddr::new(memory_map_addr), 
        memory_map_entry_count
    )
}

fn bootloader_main(
    kernel_start: PhysAddr,
    kernel_size: u64,
    memory_map_addr: VirtAddr,
    memory_map_entry_count: u64,
) -> ! {
    use memory::memory_descriptor::E820MemoryRegion;
    let e820_memory_map = {
        let ptr = memory_map_addr.as_u64() as usize as *const E820MemoryRegion;
        unsafe { core::slice::from_raw_parts(ptr, memory_map_entry_count as usize) }
    };
    let max_phys_addr = e820_memory_map
        .iter()
        .map(|r| r.start_addr + r.len)
        .max()
        .expect("no physical memory regions found");
    
    let mut frame_allocator = {
        let kernel_end = PhysFrame::containing_address(kernel_start + kernel_size - 1u64);
        let next_free = kernel_end + 1;
        LegacyFrameAllocator::new_starting_at(next_free, e820_memory_map.iter().copied())
    };

    // We identity-map all memory, so the offset between physical and virtual addresses is 0
    let phys_offset = VirtAddr::new(0);

    let mut bootloader_page_table = {
        let frame = x86_64::registers::control::Cr3::read().0;
        let table: *mut PageTable = (phys_offset + frame.start_address().as_u64()).as_mut_ptr();
        unsafe { OffsetPageTable::new(&mut *table, phys_offset) }
    };

    // identity-map remaining physical memory (first gigabyte is already identity-mapped)
    {
        let start_frame: PhysFrame<Size2MiB> =
            PhysFrame::containing_address(PhysAddr::new(4096 * 512 * 512));
        let end_frame = PhysFrame::containing_address(PhysAddr::new(max_phys_addr - 1));
        for frame in PhysFrame::range_inclusive(start_frame, end_frame) {
            unsafe {
                bootloader_page_table
                    .identity_map(
                        frame,
                        PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                        &mut frame_allocator,
                    )
                    .unwrap()
                    .flush()
            };
        }
    }

    let page_tables = create_page_tables(&mut frame_allocator);

    let framebuffer_addr = PhysAddr::new(VGA_BUFFER_START_ADDR as u64);
    let framebuffer_info = FrameBufferInfo {
        byte_len: VGA_BUFFER_SIZE,
        horizontal_resolution: VGA_TEXT_MODE_WIDTH,
        vertical_resolution: VGA_TEXT_MODE_HEIGHT,
    };
    let system_info = SystemInfo {
        framebuffer_addr,
        framebuffer_info,
        rsdp_addr: detect_rsdp(),
    };

    let kernel_bytes = unsafe {
        & *core::ptr::slice_from_raw_parts(kernel_start.as_u64() as usize as *const u8, kernel_size as usize)
    };


    load_and_switch_to_kernel(kernel_bytes, frame_allocator, page_tables, system_info);
}

fn detect_rsdp() -> Option<PhysAddr> {
    use core::ptr::NonNull;
    use rsdp::{
        handler::{AcpiHandler, PhysicalMapping},
        Rsdp,
    };

    #[derive(Clone)]
    struct IdentityMapped;
    impl AcpiHandler for IdentityMapped {
        unsafe fn map_physical_region<T>(
            &self,
            physical_address: usize,
            size: usize,
        ) -> PhysicalMapping<Self, T> {
            PhysicalMapping::new(
                physical_address, 
                NonNull::new(physical_address as *mut _).unwrap(), 
                size, 
                size, 
                self.clone()
            )
        }

        fn unmap_physical_region<T>(_region: &PhysicalMapping<Self, T>) {}
    }

    unsafe {
        Rsdp::search_for_on_bios(IdentityMapped)
            .ok()
            .map(|mapping| PhysAddr::new(mapping.physical_start() as u64))
    }
}

/// Creates page table abstraction types for both the bootloader and kernel page tables.
fn create_page_tables(
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> crate::memory::PageTables {
    // We identity-mapped all memory, so the offset between physical and virtual addresses is 0
    let phys_offset = VirtAddr::new(0);

    // copy the currently active level 4 page table, because it might be read-only
    let bootloader_page_table = {
        let frame = x86_64::registers::control::Cr3::read().0;
        let table: *mut PageTable = (phys_offset + frame.start_address().as_u64()).as_mut_ptr();
        unsafe { OffsetPageTable::new(&mut *table, phys_offset) }
    };

    // create a new page table hierarchy for the kernel
    let (kernel_page_table, kernel_level_4_frame) = {
        // get an unused frame for new level 4 page table
        let frame: PhysFrame = frame_allocator.allocate_frame().expect("no unused frames");

        log::info!("New page table at: {:#?}", &frame);

        // get the corresponding virtual address
        let addr = phys_offset + frame.start_address().as_u64();
        // initialize a new page table
        let ptr = addr.as_mut_ptr();

        unsafe { *ptr = PageTable::new() };
        let level_4_table = unsafe { &mut *ptr };
        (
            unsafe { OffsetPageTable::new(level_4_table, phys_offset) },
            frame,
        )
    };

    crate::memory::PageTables {
        bootloader: bootloader_page_table,
        kernel: kernel_page_table,
        kernel_level_4_frame,
    }
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
        unsafe { asm!("cli; hlt;") }
    }
}