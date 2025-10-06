MEMORY {
    FLASH    : ORIGIN = 0x08006000, LENGTH = 1M
    RAM      : ORIGIN = 0x20082000, LENGTH = 1536K - 8K
    SHAREDRTT : ORIGIN = 0x20081800, LENGTH = 2K
}

SECTIONS {
    .shared_rtt : {
        . = ALIGN(4);
        KEEP(* (.shared_rtt.header))
        KEEP(* (.shared_rtt.buffer))
        . = ALIGN(4);
    } > SHAREDRTT
}
