# .boot 定义在 linker.ld, 紧接着 _rest_of_bootloader_start_addr
.section .boot, "awx"
.code32

# stage3
# 1. 检查 cpu 是否支持 CPUID 指令                          [√]
# 2. 检查 cpu 是否支持 long mode                           [√]
# 3. 建立四级页表, 开启内存分页管理                         [√]
# 4. 进入 long mode , 跳转到 stage_4(定义在 src/main.rs)   [√]

stage3:
    mov esi, offset stage3_start_str
    mov ecx, 12
    call print_protected_mode

# 检查是否支持 CPUID 指令
# ref: https://wiki.osdev.org/CPUID
# 方法: testing the 'ID' bit (第 21 位) in eflags
#      This bit is modifiable only when the CPUID instruction is supported. 
#      For systems that don't support CPUID, changing the 'ID' bit will have no effect.
check_cpuid:
    pushfd       # 将 EFLAGS 压栈
    pop eax      # 保存到 eax 中
    mov ecx, eax # 然后复制到 ecx 里

    xor eax, (1 << 21) # 改变 ID 位

    push eax # 将改变后的 EFLAGES 压栈
    popfd    # 通过该指令修改 EFLAGES 寄存器

    # 再次将 EFLAGES 的值读到 eax 中
    pushfd
    pop eax

    # 还原 EFLAGES
    push ecx
    popfd

    # 再比较 EFLAGES 的值, 看前后是否发生变化
    cmp eax, ecx
    je no_cpuid # 如果相等就表示没有变化, 也就表明不支持 cpuid 指令

# 检查是否支持长模式
# ref: https://www.amd.com/system/files/TechDocs/24594.pdf
#      https://wiki.osdev.org/Long_Mode#How_do_I_detect_if_the_CPU_is_64_bits_.3F
# 方法: 1. 先检查 cpuid 支持的最大拓展函数(使用 0x8000_0000 拓展函数), 
#          如果小于 0x8000_0001 就表明 cpu 太老了
#       2. 再通过 0x8000_0001 拓展函数的返回值的第 29 位检测是否支持长模式, 
#          如果支持该位会被置为 1
check_long_mode:
    # cpuid 指令通过 eax 传参

    # Function 8000_0000h—Maximum Extended Function Number and Vendor String
    # The value returned in EAX provides the largest extended function number supported by the processor.
    mov eax, 0x80000000
    cpuid
    cmp eax, 0x80000001
    jle no_long_mode # cpu 太老

    # Function 8000_0001h—Extended Processor and Processor Feature Identifiers
    # CPUID Fn8000_0001_EDX Feature Identifiers
    # 29 LM Long mode. See “Processor Initialization and Long-Mode Activation” in APM2
    # https://www.amd.com/system/files/TechDocs/24593.pdf pdf 的第 538页甚至给出了代码
    mov eax, 0x80000001
    cpuid
    test edx, (1 << 29) # 检查第 29 位 是否被置为 1
    jz no_long_mode

    mov esi, offset support_lm_str
    mov ecx, 17
    call print_protected_mode

load_zero_idt:
    cli                   # disable interrupts
    lidt zero_idt         # Load a zero length IDT so that any NMI causes a triple fault.

# 建立 4 级页表
# ref: https://cdrdv2.intel.com/v1/dl/getContent/671200 vol.3 4 Paging
# 
# A logical processor uses 4-level paging if CR0.PG = 1, CR4.PAE = 1, IA32_EFER.LME = 1, and CR4.LA57 = 0. 
# 4-level paging translates 48-bit linear addresses to 52-bit physical addresses.1 Although 52 bits corresponds to 
# 4 PBytes, linear addresses are limited to 48 bits; at most 256 TBytes of linear-address space may be accessed 
# at any given time.
# ...
set_up_page_tables:
    # 将 page tables 对应的内存空间置 0
    # 页表相应的符号定义在 linker.ld 中
    mov edi, offset __page_table_start
    mov ecx, offset __page_table_end
    sub ecx, edi
    shr ecx, 2 # one stosd zeros 4 bytes -> divide by 4
    xor eax, eax
    rep stosd

    # ref: https://cdrdv2.intel.com/v1/dl/getContent/671200 vol.3  
    # p4
    # Table 4-15. Format of a PML4 Entry (PML4E) that References a Page-Directory-Pointer Table
    mov eax, offset _p3
    or eax, ((1 << 0) | (1 << 1)) # persent R/W
    mov [_p4], eax
    # p3
    # Table 4-17. Format of a Page-Directory-Pointer-Table Entry (PDPTE) that References a Page Directory
    mov eax, offset _p2
    or eax, ((1 << 0) | (1 << 1)) # persent R/W
    mov [_p3], eax
    # p2
    # Table 4-19. Format of a Page-Directory Entry that References a Page Table
    # Table 4-18. Format of a Page-Directory Entry that Maps a 2-MByte Page
    # 
    # The global flag signals to the hardware that a page is available in all address spaces 
    # and thus does not need to be removed from the translation cache (see the section about the TLB below) on address space switches. 
    # This flag is commonly used together with a cleared user accessible flag to **map the kernel code** to all address spaces.
    mov eax, ((1 << 0) | (1 << 1) | (1 << 7)) # persent(to map a 2-MByte Page) R/W
    # 为内核代码分配 512 个 2 MBytes 的页面, 共 1 GB 内存空间
    mov ecx, 0
    map_p2_table:
    mov [_p2 + ecx * 8], eax
    add eax, 0x200000
    add ecx, 1
    cmp ecx, 512
    jb map_p2_table

    # p1
    # 暂时为空
    # Table 4-20. Format of a Page-Table Entry that Maps a 4-KByte Page

# 开启内存分页
enable_paging:
    # Write back cache and add a memory fence. I'm not sure if this is
    # necessary, but better be on the safe side.
    wbinvd
    mfence

    # load P4 to cr3 register (cpu uses this to access the P4 table)
    mov eax, offset _p4
    mov cr3, eax

    # enable PAE-flag in cr4 (Physical Address Extension)
    mov eax, cr4
    or eax, (1 << 5)
    mov cr4, eax

    # set the long mode bit in the EFER MSR (model specific register)
    mov ecx, 0xC0000080
    rdmsr
    or eax, (1 << 8)
    wrmsr

    # enable paging in the cr0 register
    mov eax, cr0
    or eax, (1 << 31)
    mov cr0, eax

load_64bit_gdt:
    lgdt gdt_64_pointer

jump_to_long_mode:
    push CODE_SELECTOR
    mov eax, offset stage_4
    push eax
    retf # Load CS with 64 bit segment and flush the instruction cache

_spin:
    jmp _spin

no_cpuid:
    mov esi, offset no_cpuid_str
    mov ecx, 33
    call print_protected_mode
    jmp _spin

no_long_mode:
    mov esi, offset no_long_mode_str
    mov ecx, 37
    call print_protected_mode
    jmp _spin

# --------------------- data ------------------------------
no_cpuid_str:     .ascii "Error: CPU does not support CPUID"
stage3_start_str: .ascii "Booting(3)  "
no_long_mode_str: .ascii "Error: CPU does not support long mode"
support_lm_str:   .ascii "Support long mode"

.align 4
zero_idt:
    .word 0
    .byte 0

# https://wiki.osdev.org/GDT
# 要注意:
# L: Long-mode code flag. If set (1), the descriptor defines a 64-bit code segment. 
# When set, DB should always be clear. For any other type of segment (other code types or any data segment), it should be clear (0).
# 
# In 64-bit mode, the Base and Limit values are ignored, 
# each descriptor covers the entire linear address space regardless of what they are set to.
# 所以段基址和段界限都设为 0, 所以也 videodesc 没有存在的必要了
.align 4
gdt64:
    # entry 0 is always unused
    .quad 0
codedesc64:            # 代码段描述符
    .word 0            # 段界限 
    .word 0            # 段基址
    .byte 0            # (这两个字段都很分散)
    .byte 0x9a         # 段属性: a  -> Execute/Read, 段粒度: 4k -> G(1), 段权限: 0  -> DPL(00), P(1)
    .byte 0x20         # G(1) -> 粒度为 4k (段界限只给出了数值, 还要乘上相应的粒度单位), L(1)
    .byte 0            # 
datadesc64:
    .word 0            # 数据段描述符
    .word 0       
    .byte 0            
    .byte 0x92         # 段属性: 2 -> Read/Write
    .byte 0x00         # 其它同 代码段
    .byte 0
gdt64_end:

.align 4
    .word 0                              # Padding to make the "address of the GDT" field aligned on a 4-byte boundary

gdt_64_pointer:
    .word gdt_64_pointer - gdt64 - 1    # 16-bit Size (Limit) of GDT.
    .long gdt64                            # 32-bit Base Address of GDT. (CPU will zero extend to 64-bit)