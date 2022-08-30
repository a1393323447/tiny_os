# .boot 定义在 linker.ld, 紧接着 _rest_of_bootloader_start_addr
.section .boot, "awx"
.code16

# stage2
# 1. 通过 int 0x15 e820 中断, 获取内存布局, 并保存    [√] 
# 2. 从硬盘加载内核到内存                             [√]
#    方法: 通过硬盘端口读取硬盘, 通过位于 0x0500 处的 _kernel_buf 将内核 ELF 文件加载到内存 0x400000 处
# 3. 再次进入保护模式, 设置代码段选择子, 跳转到 stage3 [√] 
#   | stage_2_real.s |     stage_2_protected.s      | 

stage2:
    mov si, offset stage2_start_str
    mov ecx, 12
    call print_real_mode


set_target_operating_mode:
    # Some BIOSs assume the processor will only operate in Legacy Mode. We change the Target
    # Operating Mode to "Long Mode Target Only", so the firmware expects each CPU to enter Long Mode
    # once and then stay in it. This allows the firmware to enable mode-specifc optimizations.
    # We save the flags, because CF is set if the callback is not supported (in which case, this is
    # a NOP)
    pushf
    mov ax, 0xec00
    mov bl, 0x2
    int 0x15
    popf

create_memory_map:
    clc  # 清除 CF 标志位
    lea di, es:[_memory_map] # 这个符号在 linker.ld 中, 用来表示 memory map 在内存中的起始位置
    call do_e820
    jc not_support_e820

# ----------- Load kernel from disk ------------
# 下面的代码更优雅的方式是传递一个数据结构, 以后可以重构
load_kernel_from_disk:
    # 内核 ELF 文件先会从硬盘加载到 0x0500
    # 再从 0x0500 复制到 0x400000
    # 初始化当前 kernel ELF 复制到的位置
    mov edx, KERNEL_CODE_START_ADDR

    # 从磁盘读出数据存放的位置
    mov edi, offset _kernel_buffer

    # 计算 LBA 号
    # 计算 kernel ELF 在硬盘上的起始位置(起始扇区)
    # 在 linker.ld 中, kernel 和 bootloader 紧密排列
    # 而在链接的时候, _kernel_start_addr 这个符号也被重定位到以 0x7c00 为基地址
    # 所以 _kernel_start_addr - _start(定义在 stage_1.s) = kernel ELF 开始之前的字节数
    # 将该值除以 512 就可以得到 kernel 的起始扇区
    mov ebx, offset _kernel_start_addr
    mov ecx, offset _start
    sub ebx, ecx
    shr ebx, 9 # divide by 512 (block size) , 此时 ebx 中为 LBA 号

    # 计算扇区数
    mov ecx, offset _kernel_size
    add ecx, 511 # 向上取整
    shr ecx, 9

    # 下面是读取和复制的循环
    read_disk:
    push ecx # 保存扇区数

    # 每次读一个扇区, 因为 kernel_buffer 的大小刚好为 512 bytes
    mov ecx, 1

    # 读取硬盘
    call port_read

    # LBA 号 + 1
    add ebx, 1

    from_kernel_buf_to_kernel_code_start_addr:
    mov esi, offset _kernel_buffer
    # 获得当前 kernel ELF 复制到的位置
    mov edi, edx

    # 循环复制
    # 先设置循环次数
    mov ecx, 512 / 4 # 每次复制 4 个字节, 复制一个扇区
    rep movsd [edi], [esi]

    # 读取扇区数
    pop ecx

    # 保存当前 kernel ELF 复制到的位置
    mov edx, edi

    # 再次设置硬盘读取缓冲区
    mov edi, offset _kernel_buffer

    # 循环
    loop read_disk

# 再次进入保护模式
enter_protected_mode_again:
    cli
    lgdt [gdt32info] # 加载全局描述符表
    
    # 将 cr0 的 PE 位设置为 1 , 进入保护模式
    mov eax, cr0
    or  eax, 1
    mov cr0, eax

    push CODE_SELECTOR              # 通过 retf
    mov eax, offset protected_mode  # 设置 CS = CODE_SELECTOR
    push eax                        # IP = protected_mode
    retf                            # jmp dword CODE_SELECTOR:protected_mode

# 错误处理
not_support_e820:
    mov si, offset error
    mov cx, 11
    call print_real_mode

error_lock:
    jmp error_lock

# ------------------- func -------------------

# From http://wiki.osdev.org/Detecting_Memory_(x86)#Getting_an_E820_Memory_Map

# use the INT 0x15, eax= 0xE820 BIOS function to get a memory map
# inputs: es:di -> destination buffer for 24 byte entries
# outputs: bp = entry count, trashes all registers except esi
do_e820:
	xor ebx, ebx		    # ebx must be 0 to start
	xor bp, bp		        # keep an entry count in bp
	mov edx, 0x0534D4150	# Place "SMAP" into edx
	mov eax, 0xe820
	mov dword ptr es:[di + 20], 1	# force a valid ACPI 3.X entry
	mov ecx, 24		                # ask for 24 bytes
	int 0x15
	jc .failed	            # carry set on first call means "unsupported function"
	mov edx, 0x0534D4150	# Some BIOSes apparently trash this register?
	cmp eax, edx		    # on success, eax must have been reset to "SMAP"
	jne .failed
	test ebx, ebx		    # ebx = 0 implies list is only 1 entry long (worthless)
	je .failed
	jmp .jmpin
.e820lp:
	mov eax, 0xe820		    # eax, ecx get trashed on every int 0x15 call
	mov dword ptr es:[di + 20], 1	# force a valid ACPI 3.X entry
	mov ecx, 24		        # ask for 24 bytes again
	int 0x15
	jc .e820f		        # carry set means "end of list already reached"
	mov edx, 0x0534D4150	# repair potentially trashed register
.jmpin:
	jcxz .skipent		    # skip any 0 length entries
	cmp cl, 20		        # got a 24 byte ACPI 3.X response?
	jbe .notext
	test byte ptr es:[di + 20], 1	# if so: is the "ignore this data" bit clear?
	je .skipent
.notext:
	mov ecx, es:[di + 8]	# get lower uint32_t of memory region length
	or ecx, es:[di + 12]	# "or" it with upper uint32_t to test for zero
	jz .skipent		        # if length uint64_t is 0, skip entry
	inc bp			        # got a good entry: ++count, move to next storage spot
	add di, 24
.skipent:
	test ebx, ebx		    # if ebx resets to 0, list is complete
	jne .e820lp
.e820f:
	mov [mmap_ent], bp	    # store the entry count
	clc			            # there is "jc" on end of list to this point, so the carry must be cleared
	ret
.failed:
	stc			            # "function unsupported" error exit
	ret

# --------------------- data ------------------------------
mmap_ent:         .word 0
stage2_start_str: .ascii "Booting(2.1)"
error:            .ascii "e820 failed"

