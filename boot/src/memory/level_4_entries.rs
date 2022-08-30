use core::{convert::TryInto, iter::Step};
use x86_64::{
    VirtAddr,
    structures::paging::{Page, PageTableIndex, Size4KiB},
};
use xmas_elf::program::ProgramHeader;


/// Keeps track of used entries in a level 4 page table.
///
/// Useful for determining a free virtual memory block, e.g. for mapping additional data.
pub struct UsedLevel4Entries {
    /// Whether an entry is in use by the kernel.
    entry_state: [bool; 512],
}

impl UsedLevel4Entries {
    pub fn new() -> Self {
        let mut used = UsedLevel4Entries {
            entry_state: [false; 512],
        };

        used.entry_state[0] = true;

        let dynamic_range_start = 0;
        // Mark everything before the dynamic range as unusable.
        let dynamic_range_start = VirtAddr::new(dynamic_range_start);
        let start_page: Page = Page::containing_address(dynamic_range_start);
        if let Some(unusable_page) = Step::backward_checked(start_page, 1) {
            for i in 0..=u16::from(unusable_page.p4_index()) {
                used.mark_p4_index_as_used(PageTableIndex::new(i));
            }
        }

        let dynamic_range_end = 0xffff_ffff_ffff_f000;
        // Mark everything after the dynamic range as unusable.
        let dynamic_range_end = VirtAddr::new(dynamic_range_end);
        let end_page: Page = Page::containing_address(dynamic_range_end);
        if let Some(unusable_page) = Step::forward_checked(end_page, 1) {
            for i in u16::from(unusable_page.p4_index())..512 {
                used.mark_p4_index_as_used(PageTableIndex::new(i));
            }
        }
        
        used
    }

    /// Marks all p4 entries in the range `[address..address+size)` as used.
    ///
    /// `size` can be a `u64` or `usize`.
    fn mark_range_as_used<S>(&mut self, address: u64, size: S)
    where
        VirtAddr: core::ops::Add<S, Output = VirtAddr>,
    {
        let start = VirtAddr::new(address);
        let end_inclusive = (start + size) - 1usize;
        let start_page = Page::<Size4KiB>::containing_address(start);
        let end_page_inclusive = Page::<Size4KiB>::containing_address(end_inclusive);

        for p4_index in u16::from(start_page.p4_index())..=u16::from(end_page_inclusive.p4_index())
        {
            self.mark_p4_index_as_used(PageTableIndex::new(p4_index));
        }
    }

    fn mark_p4_index_as_used(&mut self, p4_index: PageTableIndex) {
        self.entry_state[usize::from(p4_index)] = true;
    }

    /// Marks the virtual address range of all segments as used.
    pub fn mark_segments<'a>(
        &mut self,
        segments: impl Iterator<Item = ProgramHeader<'a>>,
        virtual_address_offset: u64,
    ) {
        for segment in segments.filter(|s| s.mem_size() > 0) {
            self.mark_range_as_used(
                segment.virtual_addr() + virtual_address_offset,
                segment.mem_size(),
            );
        }
    }

    /// Returns an unused level 4 entry and marks it as used. If `CONFIG.aslr` is
    /// enabled, this will return a random available entry.
    ///
    /// Since this method marks each returned index as used, it can be used multiple times
    /// to determine multiple unused virtual memory regions.
    pub fn get_free_entry(&mut self) -> PageTableIndex {
        // Create an iterator over all available p4 indices.
        let mut free_entries = self
            .entry_state
            .iter()
            .copied()
            .enumerate()
            .filter(|(_, used)| !used)
            .map(|(idx, _)| idx);

        // Choose the free entry index.;
        let idx_opt = free_entries.next();
        let idx = idx_opt.expect("no usable level 4 entry found");

        // Mark the entry as used.
        self.entry_state[idx] = true;

        PageTableIndex::new(idx.try_into().unwrap())
    }

    /// Returns a virtual address in an unused level 4 entry and marks it as used.
    ///
    /// This function calls [`get_free_entry`] internally, so all of its docs applies here
    /// too.
    pub fn get_free_address(&mut self, _size: u64, alignment: u64) -> VirtAddr {
        assert!(alignment.is_power_of_two());

        let base =
            Page::from_page_table_indices_1gib(self.get_free_entry(), PageTableIndex::new(0))
                .start_address();

        let offset: u64 = 0;

        base + offset
    }
}