pub mod memory_descriptor;
pub mod legacy_memory_region;
pub mod level_4_entries;

use x86_64::structures::paging::{OffsetPageTable, PhysFrame};

pub const PAGE_SIZE: u64 = 4096;

/// Provides access to the page tables of the bootloader and kernel address space.
#[derive(Debug)]
pub struct PageTables {
    /// Provides access to the page tables of the bootloader address space.
    pub bootloader: OffsetPageTable<'static>,
    /// Provides access to the page tables of the kernel address space (not active).
    pub kernel: OffsetPageTable<'static>,
    /// The physical frame where the level 4 page table of the kernel address space is stored.
    ///
    /// Must be the page table that the `kernel` field of this struct refers to.
    ///
    /// This frame is loaded into the `CR3` register on the final context switch to the kernel.  
    pub kernel_level_4_frame: PhysFrame,
}