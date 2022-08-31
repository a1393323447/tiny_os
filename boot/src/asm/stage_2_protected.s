# -------------- 进入保护模式 -----------------
# ----------------- 32 位 --------------------
.code32
protected_mode:
    mov eax, DATA_SELECTOR
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov fs, ax
    mov gs, ax
    mov esp, 0x7c00

    mov esi, offset protected_str
    mov ecx, 12
    call print_protected_mode

    jmp stage3

# ------------------- func -------------------
print_protected_mode:
    push ax
    push es

    mov ax, VIDEO_SELECTOR
    mov es, ax
    xor di, di

    pprint:
    lodsb al, byte ptr ds:[si]
    # 上一条语句等价于:
    # mov al, byte ptr ds:[si]
    # inc si
    stosb
    # 上一条语句等价于:
    # mov byte ptr es:[di], al
    # inc di
    mov al, CHAR_ATTR
    stosb
    # 上一条的语句等价于:
    # mov byte ptr es:[di], CHAR_ATTR
    # inc di
    loop pprint

    pop es
    pop ax
    
    ret

# --------------------- data ------------------------------
protected_str:    .asciz "Booting(2.2)"