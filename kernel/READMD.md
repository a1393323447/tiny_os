# kernel
## 简介
`kernel` 阶段任在开发中, 现在主要完成了:
- 通过 `BootInfo` 对内核进行初始化:
    - 配置内核的 `GDT` , 包括
        - 设置 `kernel` 的代码段的段选择子
        - 设置 `TSS` 段选择子 (为了配置中断栈)
    - 初始化 `IDT` , 主要处理了
        - [`Breakpoint`](https://wiki.osdev.org/Exceptions#Breakpoint)
        - [`Double Fault`](https://wiki.osdev.org/Exceptions#Double_Fault)
        - [`General Protection Fault`](https://wiki.osdev.org/Exceptions#General_Protection_Fault)
        - [`Page Fault`](https://wiki.osdev.org/Exceptions#Page_Fault)
    - 初始化 `8259 PIC`, 并处理了:
        - 时钟中断
        - 键盘中断
    - 初始化 `logger` , 用于打印信息
    - 启用中断
- 通过 `BootInfo` 中提供的 `memory_map` 构建了一个物理页分配器
- 通过读取 `cr3` 寄存器获得当前页表的地址, 并通过 `OffsetPageTable` 结构管理物理地址和虚拟地址的映射关系
- 分配了一个内核的堆空间, 并通过 `linked_list_allocator` 动态分配和释放该空间
- 实现了一个 `Executor` 管理协作式任务, 并为键盘 IO 开启了一个协作式任务