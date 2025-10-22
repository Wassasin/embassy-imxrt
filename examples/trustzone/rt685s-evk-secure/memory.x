MEMORY {
    FLASH     : ORIGIN = 0x10170000, LENGTH = 20k,
    RAM       : ORIGIN = 0x30176000, LENGTH = 8K,
    SHAREDRTT : ORIGIN = 0x30178000, LENGTH = 2K
}

SECTIONS {
    .shared_rtt ORIGIN(SHAREDRTT) (NOLOAD): {
        . = ALIGN(4);
        KEEP(* (.shared_rtt.header))
        KEEP(* (.shared_rtt.buffer))
        . = ALIGN(4);
    } > SHAREDRTT
}
