# 这个文件中记录的是通过 0x13 中断读取硬盘, 加载内核的代码,
# 经过多次重构, 在 bootloader 中被淘汰

load_kernel_from_disk:
    # 在 linker.ld 中定义的一个用于加载内核的 buffer
    # 起始地址为 0x0500, 大小为 512 bytes
    mov eax, offset _kernel_buffer
    mov [dap_buf_offset], ax

    # 每次加载一个扇区, 也就是 512 bytes
    mov word ptr [dap_sector_num], 1

    # 计算 kernel 代码在硬盘上的起始位置(起始扇区)
    # 在 linker.ld 中, kernel 和 bootloader 紧密排列
    # 而在链接的时候, _kernel_start_addr 这个符号也被重定位到以 0x7c00 为基地址
    # 所以 _kernel_start_addr - _start(定义在 stage_1.s) = kernel代码开始之前的字节数
    # 将该值除以 512 就可以得到 kernel 的起始扇区
    mov eax, offset _kernel_start_addr
    mov ebx, offset _start
    sub eax, ebx
    shr eax, 9 # divide by 512 (block size)
    mov [dap_start_lba], eax

    # 内核代码要加载到 0x400000 处
    mov edi, 0x400000

    # 计算循环的次数
    mov ecx, offset _kernel_size
    add ecx, 511 # 向上取整
    shr ecx, 9

load_next_kernel_block_from_disk:
    # load block from disk
    mov si, offset dap
    mov ah, 0x42
    int 0x13
    jc load_next_kernel_block_from_disk

    # copy block to 4MiB (0x400000)
    push ecx
    push esi
    # move with zero extension
    # 一个 word 是 16 位, 而 esi 是 32 位
    # 所以要用 movzx 指令, 高 16 位会被置为 0
    movzx esi, word ptr [dap_buf_offset]
    # ecx 作为循环次数, 每次复制 32 位(4 个字节)
    mov ecx, 512 / 4
    rep movsd [edi], [esi]
    pop esi
    pop ecx

    # next block
    mov eax, [dap_start_lba]
    add eax, 1
    mov [dap_start_lba], eax

    sub ecx, 1
    jnz load_next_kernel_block_from_disk
