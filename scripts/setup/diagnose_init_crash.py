#!/usr/bin/env python3
"""
Diagnose the Linux init process SIGSEGV crash.

Phase 1-2 from previous runs established the crash location:
  - User mode starts at ~cycle 154,723,095
  - Fatal trap at ~cycle 154,723,444
  - Last user PC: 0x3f89d0800a
  - a5 loaded as 0 at PC 0x3f89d07ffe (should be non-zero)

This version uses read_memory, read_csr, and get_privilege
to deeply inspect the crash.

Run from repo root:
  sim script scripts/setup/diagnose_init_crash.py
"""

import os
import sys
import time

REG_NAMES = [
    "zero",
    "ra",
    "sp",
    "gp",
    "tp",
    "t0",
    "t1",
    "t2",
    "s0",
    "s1",
    "a0",
    "a1",
    "a2",
    "a3",
    "a4",
    "a5",
    "a6",
    "a7",
    "s2",
    "s3",
    "s4",
    "s5",
    "s6",
    "s7",
    "s8",
    "s9",
    "s10",
    "s11",
    "t3",
    "t4",
    "t5",
    "t6",
]


def read_all_regs(cpu):
    return tuple(cpu.read_register(i) for i in range(32))


def fmt_reg(idx, val):
    return f"x{idx:<2} ({REG_NAMES[idx]:>4}) = 0x{val:016x}"


def get_user_cycles(cpu):
    return cpu.get_stats().to_dict()["cycles_user"]


# ---------- Minimal RISC-V instruction decoder ----------


def decode_rv_inst(word, pc, is_compressed=False):
    """Decode a RISC-V instruction to a human-readable string."""
    if is_compressed:
        return decode_compressed(word & 0xFFFF, pc)
    return decode_32bit(word, pc)


def decode_compressed(hw, pc):
    """Decode a 16-bit compressed instruction."""
    op = hw & 0x3
    funct3 = (hw >> 13) & 0x7

    if op == 0:  # C0
        if funct3 == 0:  # C.ADDI4SPN
            rd = 8 + ((hw >> 2) & 0x7)
            return f"c.addi4spn {REG_NAMES[rd]}, sp, ..."
        if funct3 == 2:  # C.LW
            return "c.lw ..."
        if funct3 == 3:  # C.LD
            rs1 = 8 + ((hw >> 7) & 0x7)
            rd = 8 + ((hw >> 2) & 0x7)
            imm = ((hw >> 10) & 0x7) << 3 | ((hw >> 5) & 0x3) << 6
            return f"c.ld {REG_NAMES[rd]}, {imm}({REG_NAMES[rs1]})"
        if funct3 == 6:  # C.SW
            return "c.sw ..."
        if funct3 == 7:  # C.SD
            rs1 = 8 + ((hw >> 7) & 0x7)
            rs2 = 8 + ((hw >> 2) & 0x7)
            imm = ((hw >> 10) & 0x7) << 3 | ((hw >> 5) & 0x3) << 6
            return f"c.sd {REG_NAMES[rs2]}, {imm}({REG_NAMES[rs1]})"
    elif op == 1:  # C1
        if funct3 == 0:  # C.ADDI / C.NOP
            rd = (hw >> 7) & 0x1F
            imm = ((hw >> 2) & 0x1F) | (((hw >> 12) & 1) << 5)
            if imm & 0x20:
                imm |= ~0x3F  # sign extend
            if rd == 0:
                return "c.nop"
            return f"c.addi {REG_NAMES[rd]}, {imm}"
        if funct3 == 1:  # C.ADDIW
            rd = (hw >> 7) & 0x1F
            imm = ((hw >> 2) & 0x1F) | (((hw >> 12) & 1) << 5)
            if imm & 0x20:
                imm |= ~0x3F
            return f"c.addiw {REG_NAMES[rd]}, {imm}"
        if funct3 == 2:  # C.LI
            rd = (hw >> 7) & 0x1F
            imm = ((hw >> 2) & 0x1F) | (((hw >> 12) & 1) << 5)
            if imm & 0x20:
                imm |= ~0x3F
            return f"c.li {REG_NAMES[rd]}, {imm}"
        if funct3 == 3:  # C.LUI / C.ADDI16SP
            rd = (hw >> 7) & 0x1F
            if rd == 2:
                return "c.addi16sp ..."
            return f"c.lui {REG_NAMES[rd]}, ..."
        if funct3 == 4:  # ALU
            funct2 = (hw >> 10) & 0x3
            if funct2 == 0:
                return "c.srli ..."
            if funct2 == 1:
                return "c.srai ..."
            if funct2 == 2:
                return "c.andi ..."
            if funct2 == 3:
                rd = 8 + ((hw >> 7) & 0x7)
                rs2 = 8 + ((hw >> 2) & 0x7)
                sub = (hw >> 5) & 0x3
                bit12 = (hw >> 12) & 1
                if bit12 == 0:
                    ops = ["c.sub", "c.xor", "c.or", "c.and"]
                else:
                    ops = ["c.subw", "c.addw", "?", "?"]
                return f"{ops[sub]} {REG_NAMES[rd]}, {REG_NAMES[rs2]}"
        if funct3 == 5:  # C.J
            return "c.j ..."
        if funct3 == 6:  # C.BEQZ
            rs1 = 8 + ((hw >> 7) & 0x7)
            return f"c.beqz {REG_NAMES[rs1]}, ..."
        if funct3 == 7:  # C.BNEZ
            rs1 = 8 + ((hw >> 7) & 0x7)
            return f"c.bnez {REG_NAMES[rs1]}, ..."
    elif op == 2:  # C2
        if funct3 == 0:  # C.SLLI
            rd = (hw >> 7) & 0x1F
            shamt = ((hw >> 2) & 0x1F) | (((hw >> 12) & 1) << 5)
            return f"c.slli {REG_NAMES[rd]}, {shamt}"
        if funct3 == 2:  # C.LWSP
            rd = (hw >> 7) & 0x1F
            return f"c.lwsp {REG_NAMES[rd]}, ..."
        if funct3 == 3:  # C.LDSP
            rd = (hw >> 7) & 0x1F
            imm = (
                ((hw >> 2) & 0x7) << 6 | ((hw >> 12) & 1) << 5 | ((hw >> 5) & 0x3) << 3
            )
            # Actually: imm[5] = bit12, imm[4:3] = bits[6:5], imm[8:6] = bits[4:2]
            # offset = imm[5]<<5 | imm[4:3]<<3 | imm[8:6]<<6
            return f"c.ldsp {REG_NAMES[rd]}, {imm}(sp)"
        if funct3 == 4:
            bit12 = (hw >> 12) & 1
            rs1 = (hw >> 7) & 0x1F
            rs2 = (hw >> 2) & 0x1F
            if bit12 == 0:
                if rs2 == 0:
                    return f"c.jr {REG_NAMES[rs1]}"
                return f"c.mv {REG_NAMES[rs1]}, {REG_NAMES[rs2]}"
            else:
                if rs2 == 0:
                    if rs1 == 0:
                        return "c.ebreak"
                    return f"c.jalr {REG_NAMES[rs1]}"
                return f"c.add {REG_NAMES[rs1]}, {REG_NAMES[rs2]}"
        if funct3 == 6:  # C.SWSP
            return "c.swsp ..."
        if funct3 == 7:  # C.SDSP
            rs2 = (hw >> 2) & 0x1F
            imm = ((hw >> 7) & 0x7) << 6 | ((hw >> 10) & 0x7) << 3
            # Actually: imm[5:3] = bits[12:10], imm[8:6] = bits[9:7]
            return f"c.sdsp {REG_NAMES[rs2]}, {imm}(sp)"

    return f"c.??? (0x{hw:04x})"


def decode_32bit(word, pc):
    """Decode a 32-bit RISC-V instruction."""
    opcode = word & 0x7F
    rd = (word >> 7) & 0x1F
    funct3 = (word >> 12) & 0x7
    rs1 = (word >> 15) & 0x1F
    rs2 = (word >> 20) & 0x1F
    funct7 = (word >> 25) & 0x7F

    # I-type immediate
    def imm_i():
        v = (word >> 20) & 0xFFF
        if v & 0x800:
            v |= ~0xFFF  # sign extend
        return v

    # S-type immediate
    def imm_s():
        v = ((word >> 7) & 0x1F) | (((word >> 25) & 0x7F) << 5)
        if v & 0x800:
            v |= ~0xFFF
        return v

    # B-type immediate
    def imm_b():
        v = (
            (((word >> 8) & 0xF) << 1)
            | (((word >> 25) & 0x3F) << 5)
            | (((word >> 7) & 1) << 11)
            | (((word >> 31) & 1) << 12)
        )
        if v & 0x1000:
            v |= ~0x1FFF
        return v

    width_names = {0: "b", 1: "h", 2: "w", 3: "d", 4: "bu", 5: "hu", 6: "wu"}

    if opcode == 0x03:  # LOAD
        wn = width_names.get(funct3, "?")
        off = imm_i()
        return f"l{wn} {REG_NAMES[rd]}, {off}({REG_NAMES[rs1]})"

    if opcode == 0x23:  # STORE
        wn = {0: "b", 1: "h", 2: "w", 3: "d"}.get(funct3, "?")
        off = imm_s()
        return f"s{wn} {REG_NAMES[rs2]}, {off}({REG_NAMES[rs1]})"

    if opcode == 0x13:  # OP-IMM
        ops = {
            0: "addi",
            1: "slli",
            2: "slti",
            3: "sltiu",
            4: "xori",
            5: "srli/srai",
            6: "ori",
            7: "andi",
        }
        return f"{ops.get(funct3, '?')} {REG_NAMES[rd]}, {REG_NAMES[rs1]}, {imm_i()}"

    if opcode == 0x1B:  # OP-IMM-32
        ops = {0: "addiw", 1: "slliw", 5: "srliw/sraiw"}
        return (
            f"{ops.get(funct3, 'op32?')} {REG_NAMES[rd]}, {REG_NAMES[rs1]}, {imm_i()}"
        )

    if opcode == 0x33:  # OP
        if funct7 == 1:  # M extension
            ops = {
                0: "mul",
                1: "mulh",
                2: "mulhsu",
                3: "mulhu",
                4: "div",
                5: "divu",
                6: "rem",
                7: "remu",
            }
            return f"{ops.get(funct3, '?')} {REG_NAMES[rd]}, {REG_NAMES[rs1]}, {REG_NAMES[rs2]}"
        ops = {
            0: "add" if funct7 == 0 else "sub",
            1: "sll",
            2: "slt",
            3: "sltu",
            4: "xor",
            5: "srl" if funct7 == 0 else "sra",
            6: "or",
            7: "and",
        }
        return f"{ops.get(funct3, '?')} {REG_NAMES[rd]}, {REG_NAMES[rs1]}, {REG_NAMES[rs2]}"

    if opcode == 0x3B:  # OP-32
        return f"op32 {REG_NAMES[rd]}, {REG_NAMES[rs1]}, {REG_NAMES[rs2]}"

    if opcode == 0x37:  # LUI
        imm_u = word & 0xFFFFF000
        return f"lui {REG_NAMES[rd]}, 0x{imm_u >> 12:x}"

    if opcode == 0x17:  # AUIPC
        imm_u = word & 0xFFFFF000
        if imm_u & 0x80000000:
            imm_u -= 0x100000000
        return (
            f"auipc {REG_NAMES[rd]}, 0x{(imm_u >> 12) & 0xFFFFF:x} (={pc + imm_u:#x})"
        )

    if opcode == 0x6F:  # JAL
        return f"jal {REG_NAMES[rd]}, ..."

    if opcode == 0x67:  # JALR
        return f"jalr {REG_NAMES[rd]}, {imm_i()}({REG_NAMES[rs1]})"

    if opcode == 0x63:  # BRANCH
        ops = {0: "beq", 1: "bne", 4: "blt", 5: "bge", 6: "bltu", 7: "bgeu"}
        target = pc + imm_b()
        return f"{ops.get(funct3, 'b?')} {REG_NAMES[rs1]}, {REG_NAMES[rs2]}, 0x{target & 0xFFFFFFFFFFFFFFFF:x}"

    if opcode == 0x73:  # SYSTEM
        if word == 0x00000073:
            return "ecall"
        if word == 0x00100073:
            return "ebreak"
        if word == 0x10200073:
            return "sret"
        if word == 0x30200073:
            return "mret"
        if word == 0x10500073:
            return "wfi"
        if funct3 >= 1:
            csr_num = (word >> 20) & 0xFFF
            ops = {
                1: "csrrw",
                2: "csrrs",
                3: "csrrc",
                5: "csrrwi",
                6: "csrrsi",
                7: "csrrci",
            }
            return f"{ops.get(funct3, 'csr?')} {REG_NAMES[rd]}, 0x{csr_num:03x}, {REG_NAMES[rs1]}"

    if opcode == 0x0F:  # MISC-MEM (FENCE)
        if funct3 == 0:
            return "fence"
        if funct3 == 1:
            return "fence.i"

    if opcode == 0x2F:  # AMO
        funct5 = funct7 >> 2
        ops = {
            0: "amoadd",
            1: "amoswap",
            2: "lr",
            3: "sc",
            4: "amoxor",
            8: "amomin",
            0xC: "amomax",
            0x10: "amominu",
            0x14: "amomaxu",
            0x18: "amoor",
            0x1C: "amoand",
        }
        wn = "w" if funct3 == 2 else "d"
        return f"{ops.get(funct5, 'amo?')}.{wn} {REG_NAMES[rd]}, {REG_NAMES[rs1]}, {REG_NAMES[rs2]}"

    return f"??? (0x{word:08x}, opcode=0x{opcode:02x})"


# ---------- SV39 Page Table Walker (from Python) ----------


def sv39_walk(cpu, va, satp):
    """Walk SV39 page tables to translate a virtual address."""
    mode = (satp >> 60) & 0xF
    if mode != 8:  # SV39
        return None, f"satp mode {mode} != 8 (SV39)"

    ppn = satp & 0x00000FFFFFFFFFFF
    pt_base = ppn << 12

    vpn = [(va >> 12) & 0x1FF, (va >> 21) & 0x1FF, (va >> 30) & 0x1FF]
    page_offset = va & 0xFFF

    for level in [2, 1, 0]:
        pte_addr = pt_base + vpn[level] * 8
        pte = cpu.read_memory_u64(pte_addr)

        valid = pte & 1
        if not valid:
            return (
                None,
                f"PTE invalid at level {level}, addr 0x{pte_addr:x}, pte=0x{pte:016x}",
            )

        r = (pte >> 1) & 1
        w = (pte >> 2) & 1
        x = (pte >> 3) & 1

        if r == 0 and w == 0 and x == 0:
            # Pointer to next level
            pt_base = ((pte >> 10) & 0x00000FFFFFFFFFFF) << 12
            continue

        # Leaf PTE
        pte_ppn = (pte >> 10) & 0x00000FFFFFFFFFFF
        if level == 2:  # 1GB gigapage
            pa = (pte_ppn << 12) | (va & 0x3FFFFFFF)
        elif level == 1:  # 2MB megapage
            pa = (pte_ppn << 12) | (va & 0x1FFFFF)
        else:  # 4KB page
            pa = (pte_ppn << 12) | page_offset

        flags = []
        if r:
            flags.append("R")
        if w:
            flags.append("W")
        if x:
            flags.append("X")
        if (pte >> 4) & 1:
            flags.append("U")
        if (pte >> 6) & 1:
            flags.append("A")
        if (pte >> 7) & 1:
            flags.append("D")
        page_size = ["4KB", "2MB", "1GB"][level]

        return pa, f"PA=0x{pa:x} ({page_size} {''.join(flags)}) pte=0x{pte:016x}"

    return None, "Walk exhausted without finding leaf"


def read_inst_at_va(cpu, va, satp):
    """Read instruction bytes at a virtual address using page table walk."""
    pa, info = sv39_walk(cpu, va, satp)
    if pa is None:
        return None, info
    word = cpu.read_memory_u32(pa)
    return word, info


# ---------- Config / CPU creation ----------


def optimized_config():
    from inspectre import SimConfig

    c = SimConfig.default()
    c.general.trace_instructions = False
    c.general.start_pc = 0x80000000
    c.system.ram_base = 0x80000000
    c.system.uart_base = 0x10000000
    c.system.disk_base = 0x10001000
    c.system.clint_base = 0x02000000
    c.system.syscon_base = 0x00100000
    c.system.kernel_offset = 0x200000
    c.system.bus_width = 8
    c.system.bus_latency = 1
    c.system.clint_divider = 100
    c.memory.ram_size = 256 * 1024 * 1024
    c.memory.controller = "Simple"
    c.memory.row_miss_latency = 10
    c.memory.tlb_size = 64
    c.cache.l1_i.enabled = True
    c.cache.l1_i.size_bytes = 65536
    c.cache.l1_i.line_bytes = 64
    c.cache.l1_i.ways = 8
    c.cache.l1_i.policy = "PLRU"
    c.cache.l1_i.latency = 1
    c.cache.l1_i.prefetcher = "NextLine"
    c.cache.l1_i.prefetch_degree = 2
    c.cache.l1_d.enabled = True
    c.cache.l1_d.size_bytes = 65536
    c.cache.l1_d.line_bytes = 64
    c.cache.l1_d.ways = 8
    c.cache.l1_d.policy = "PLRU"
    c.cache.l1_d.latency = 1
    c.cache.l1_d.prefetcher = "Stride"
    c.cache.l1_d.prefetch_table_size = 128
    c.cache.l1_d.prefetch_degree = 2
    c.cache.l2.enabled = True
    c.cache.l2.size_bytes = 1048576
    c.cache.l2.line_bytes = 64
    c.cache.l2.ways = 16
    c.cache.l2.policy = "PLRU"
    c.cache.l2.latency = 8
    c.cache.l2.prefetcher = "NextLine"
    c.cache.l2.prefetch_degree = 1
    c.cache.l3.enabled = True
    c.cache.l3.size_bytes = 8 * 1024 * 1024
    c.cache.l3.line_bytes = 64
    c.cache.l3.ways = 16
    c.cache.l3.policy = "PLRU"
    c.cache.l3.latency = 28
    c.cache.l3.prefetcher = "None"
    c.pipeline.branch_predictor = "TAGE"
    c.pipeline.width = 1
    c.pipeline.btb_size = 4096
    c.pipeline.ras_size = 48
    c.pipeline.tage.num_banks = 4
    c.pipeline.tage.table_size = 2048
    c.pipeline.tage.loop_table_size = 256
    c.pipeline.tage.reset_interval = 2000
    c.pipeline.tage.history_lengths = [5, 15, 44, 130]
    c.pipeline.tage.tag_widths = [9, 9, 10, 10]
    return c


def create_cpu(disk_path, image_path, dtb_path):
    from inspectre.objects import System, Cpu

    config = optimized_config()
    config.system.uart_to_stderr = True
    sys_obj = System(ram_size=config.memory.ram_size)
    sys_obj.instantiate(disk_image=disk_path, config=config)
    cpu_obj = Cpu(sys_obj, config=config)
    cpu = cpu_obj.create()
    cpu_obj.load_kernel(image_path, dtb_path)
    return cpu


def repo_root():
    return os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))


def main():
    root = repo_root()
    linux_dir = os.path.join(root, "software", "linux")
    out_dir = os.path.join(linux_dir, "output")
    image_path = os.path.join(out_dir, "Image")
    disk_path = os.path.join(out_dir, "disk.img")
    dtb_path = os.path.join(linux_dir, "system.dtb")

    for path, name in [(image_path, "Kernel"), (disk_path, "Disk"), (dtb_path, "DTB")]:
        if not os.path.isfile(path):
            print(f"Error: {name} not found at {path}")
            return 1

    os.chdir(root)

    # ────────────────────────────────────────────────────────────────────
    # Phase 1: Binary search for the crash cycle
    # ────────────────────────────────────────────────────────────────────
    CHUNK = 500_000
    FF_START = 160_000_000  # well before the crash

    print("=" * 72)
    print(
        f"  Phase 1: Fast-forward to {FF_START:,}, then search in {CHUNK:,}-cycle chunks"
    )
    print("=" * 72)

    cpu = create_cpu(disk_path, image_path, dtb_path)
    t0 = time.time()
    result = cpu.run(limit=FF_START)
    if result is not None:
        print(
            f"  ERROR: exited during fast-forward at cycle {cpu.get_stats().cycles:,}"
        )
        return 1
    print(f"  Fast-forward done in {time.time()-t0:.1f}s")

    # Run in chunks until exit
    crash_cycle = None
    while True:
        before_cycle = cpu.get_stats().cycles
        r = cpu.run(limit=CHUNK)
        after_cycle = cpu.get_stats().cycles
        pc = cpu.get_pc()
        if r is not None:
            crash_cycle = after_cycle
            print(f"  Exit detected at cycle {after_cycle:,}, PC=0x{pc:016x}")
            break

    if crash_cycle is None:
        print("  No crash detected!")
        return 1

    # ────────────────────────────────────────────────────────────────────
    # Phase 2: Re-run from just before crash, single-stepping
    # ────────────────────────────────────────────────────────────────────
    # We want to find when PC goes to a garbage address. The crash happened
    # in the range [crash_cycle - CHUNK, crash_cycle]. Restart and fast-forward
    # to crash_cycle - CHUNK, then single-step.
    MARGIN = 5_000  # extra cycles of margin
    ff2 = max(0, crash_cycle - CHUNK - MARGIN)

    print()
    print("=" * 72)
    print(f"  Phase 2: Re-run, fast-forward to {ff2:,}, then single-step")
    print("=" * 72)

    cpu = create_cpu(disk_path, image_path, dtb_path)
    t0 = time.time()
    result = cpu.run(limit=ff2)
    if result is not None:
        print(f"  ERROR: exited during fast-forward")
        return 1
    print(f"  Fast-forward done in {time.time()-t0:.1f}s")

    # Single-step watching for garbage PC
    VALID_RANGES = [
        (0x80000000, 0x90000000),  # machine mode / RAM
        (0x02000000, 0x03000000),  # CLINT
        (0xFFFFFFFF80000000, 0xFFFFFFFFFFFFFFFF + 1),  # kernel
        (0x00000001, 0x0000010000000000),  # user space (broad)
    ]

    def is_valid_pc(pc):
        for lo, hi in VALID_RANGES:
            if lo <= pc < hi:
                return True
        return False

    max_single_steps = CHUNK + MARGIN * 2
    prev_regs = read_all_regs(cpu)
    prev_pc = cpu.get_pc()

    for step in range(max_single_steps):
        result = cpu.run(limit=1)
        pc = cpu.get_pc()

        if not is_valid_pc(pc):
            cycle = cpu.get_stats().cycles
            print(f"\n  *** GARBAGE PC DETECTED at cycle {cycle:,}! ***")
            print(f"  PC jumped to: 0x{pc:016x}")
            print(f"  Previous PC:  0x{prev_pc:016x}")
            print(f"  Privilege:    {cpu.get_privilege()}")

            # Read current registers
            new_regs = read_all_regs(cpu)
            changes = []
            for i in range(32):
                if prev_regs[i] != new_regs[i]:
                    changes.append((i, REG_NAMES[i], prev_regs[i], new_regs[i]))

            if changes:
                print(f"  Register changes:")
                for i, rn, old, new in changes:
                    print(f"    {rn}: 0x{old:016x} -> 0x{new:016x}")

            # Read CSRs
            print(f"\n  CSR state:")
            for name in [
                "scause",
                "stval",
                "sepc",
                "stvec",
                "satp",
                "mstatus",
                "sstatus",
            ]:
                v = cpu.read_csr(name)
                if v is not None:
                    print(f"    {name:>10} = 0x{v:016x}")

            # Decode the instruction at prev_pc
            satp = cpu.read_csr("satp")
            print(f"\n  Instruction at previous PC 0x{prev_pc:016x}:")
            word, info = read_inst_at_va(cpu, prev_pc, satp)
            if word is not None:
                is_c = (word & 0x3) != 0x3
                asm = decode_rv_inst(word, prev_pc, is_compressed=is_c)
                print(f"    {asm}  (0x{word:08x})")
                print(f"    Page: {info}")

                cross = (prev_pc & 0xFFF) >= 0xFFE and not is_c
                if cross:
                    print(
                        f"    *** CROSS-PAGE INSTRUCTION (offset 0x{prev_pc & 0xFFF:03x})! ***"
                    )
                    # Read halves separately
                    lo_pa, lo_info = sv39_walk(cpu, prev_pc, satp)
                    hi_va = (prev_pc & ~0xFFF) + 0x1000
                    hi_pa, hi_info = sv39_walk(cpu, hi_va, satp)
                    if lo_pa is not None and hi_pa is not None:
                        lo_hw = cpu.read_memory_u32(lo_pa) & 0xFFFF
                        hi_hw = cpu.read_memory_u32(hi_pa) & 0xFFFF
                        correct_inst = (hi_hw << 16) | lo_hw
                        is_c2 = (correct_inst & 0x3) != 0x3
                        asm2 = decode_rv_inst(
                            correct_inst, prev_pc, is_compressed=is_c2
                        )
                        print(
                            f"    Correct (cross-page) encoding: 0x{correct_inst:08x}"
                        )
                        print(f"    Correct decode: {asm2}")
                        print(f"    Lower half PA: 0x{lo_pa:x} ({lo_info})")
                        print(f"    Upper half PA: 0x{hi_pa:x} ({hi_info})")
            else:
                print(f"    Cannot read: {info}")

            # Dump the PC trace for context
            print(f"\n  PC trace (last 32 committed instructions):")
            pc_trace = cpu.get_pc_trace()
            for idx, (tpc, tinst) in enumerate(pc_trace):
                is_c = (tinst & 0x3) != 0x3
                tasm = decode_rv_inst(tinst, tpc, is_compressed=is_c)
                enc = f"{tinst & 0xFFFF:04x}" if is_c else f"{tinst:08x}"
                print(f"    [{idx:2}] 0x{tpc:016x}: {enc}  {tasm}")

            # Also dump registers
            print(f"\n  Full register state:")
            for i in range(0, 32, 2):
                print(f"    {fmt_reg(i, new_regs[i])}  {fmt_reg(i+1, new_regs[i+1])}")
            return 0

        prev_pc = pc
        prev_regs = read_all_regs(cpu)

        if result is not None:
            print(f"\n  Sim exited at step {step}")
            break

    print("  Did not find garbage PC in single-step window")

    # Fall through to CSR/register analysis
    result = None

    # ── CSR state ──────────────────────────────────────────────────────
    print()
    print("=" * 72)
    print("  CSR STATE AT EXIT")
    print("=" * 72)
    csrs = {}
    for name in [
        "scause",
        "stval",
        "sepc",
        "stvec",
        "satp",
        "mcause",
        "mtval",
        "mepc",
        "mtvec",
        "mstatus",
        "sstatus",
        "medeleg",
        "mideleg",
    ]:
        v = cpu.read_csr(name)
        if v is not None:
            csrs[name] = v
            extra = ""
            if name == "scause":
                code = v & 0x7FFFFFFFFFFFFFFF
                cause_names = {
                    0: "Inst addr misaligned",
                    1: "Inst access fault",
                    2: "Illegal inst",
                    3: "Breakpoint",
                    4: "Load addr misaligned",
                    5: "Load access fault",
                    12: "Inst page fault",
                    13: "Load page fault",
                    15: "Store page fault",
                }
                extra = f"  ({'IRQ' if (v>>63) else 'EXC'}: {cause_names.get(code, f'code {code}')})"
            print(f"  {name:>10} = 0x{v:016x}{extra}")

    # ── Register state ─────────────────────────────────────────────────
    print()
    print("=" * 72)
    print("  REGISTER STATE AT EXIT")
    print("=" * 72)
    regs = read_all_regs(cpu)
    for i in range(0, 32, 2):
        print(f"    {fmt_reg(i, regs[i])}  {fmt_reg(i+1, regs[i+1])}")

    # ── PC trace (last 32 committed instructions) ──────────────────────
    print()
    print("=" * 72)
    print("  PC TRACE (last 32 committed instructions)")
    print("=" * 72)

    satp = csrs.get("satp", 0)
    pc_trace = cpu.get_pc_trace()

    for idx, (pc, inst_raw) in enumerate(pc_trace):
        is_c = (inst_raw & 0x3) != 0x3
        asm = decode_rv_inst(inst_raw, pc, is_compressed=is_c)
        enc = f"{inst_raw & 0xFFFF:04x}" if is_c else f"{inst_raw:08x}"
        marker = " <<<" if idx == len(pc_trace) - 1 else ""
        print(f"  [{idx:2}] 0x{pc:016x}: {enc}  {asm}{marker}")

    # ── Identify the bad jump ──────────────────────────────────────────
    # Look for a jump/ret to a garbage address in the trace
    print()
    print("=" * 72)
    print("  ANALYSIS")
    print("=" * 72)

    KERNEL_LO = 0xFFFFFFFF80000000
    KERNEL_HI = 0xFFFFFFFFFFFFFFFF
    MACHINE_LO = 0x80000000
    MACHINE_HI = 0x82200000

    bad_idx = None
    for idx, (pc, _) in enumerate(pc_trace):
        if not (
            (KERNEL_LO <= pc <= KERNEL_HI)
            or (MACHINE_LO <= pc <= MACHINE_HI)
            or (0x2000000 <= pc < 0x3000000)
            or (pc < 0x100000000 and pc > 0x10000)
        ):
            # PC is garbage – previous entry is the culprit
            bad_idx = idx
            break

    if bad_idx is not None and bad_idx > 0:
        prev_pc, prev_inst = pc_trace[bad_idx - 1]
        bad_pc = pc_trace[bad_idx][0]
        is_c = (prev_inst & 0x3) != 0x3
        asm = decode_rv_inst(prev_inst, prev_pc, is_compressed=is_c)
        print(f"  Jump to garbage 0x{bad_pc:016x} originated from:")
        print(f"    PC  = 0x{prev_pc:016x}")
        print(f"    Inst = {asm}  (0x{prev_inst:08x})")

        # Try to read the instruction from memory via page table walk
        if satp:
            word, info = read_inst_at_va(cpu, prev_pc, satp)
            if word is not None:
                is_c2 = (word & 0x3) != 0x3
                asm2 = decode_rv_inst(word, prev_pc, is_compressed=is_c2)
                print(f"    Re-read from memory: {asm2}  (0x{word:08x})")
                if word != prev_inst:
                    cross = (prev_pc & 0xFFF) >= 0xFFE and not is_c2
                    print(
                        f"    *** ENCODING MISMATCH! Fetched 0x{prev_inst:08x} vs memory 0x{word:08x} ***"
                    )
                    if cross:
                        print(
                            f"    *** Instruction at page boundary offset 0x{prev_pc & 0xFFF:03x} ***"
                        )
                else:
                    print(f"    Encoding matches memory.")
            else:
                print(f"    Cannot re-read: {info}")
    elif bad_idx == 0:
        print(f"  First trace entry is already garbage: 0x{pc_trace[0][0]:016x}")
    else:
        print(f"  No garbage PC detected in trace. The crash may have been")
        print(f"  detected after the oops handler ran (all traced PCs are kernel).")
        # In this case, look for the scause/sepc from the original fault
        sepc = csrs.get("sepc", 0)
        scause = csrs.get("scause", 0)
        stval = csrs.get("stval", 0)
        print(f"  scause=0x{scause:x} sepc=0x{sepc:016x} stval=0x{stval:016x}")

    return 0


def dump_full_diagnosis(cpu, crash_session, all_sessions):
    """Print full diagnosis with instruction decode and memory inspection."""

    satp = crash_session.get("satp", 0)
    csrs = crash_session.get("csrs", {})
    trace = crash_session.get("trace", [])
    trap_regs = crash_session.get("trap_regs", ())
    entry_regs = crash_session.get("entry_regs", ())
    entry_pc = crash_session.get("entry_pc", 0)
    trap_pc = crash_session.get("trap_pc", 0)

    print()
    print("=" * 72)
    print("  CSR STATE AT TRAP")
    print("=" * 72)
    for name in [
        "scause",
        "stval",
        "sepc",
        "stvec",
        "satp",
        "mcause",
        "mtval",
        "mepc",
        "mtvec",
        "mstatus",
        "sstatus",
        "medeleg",
    ]:
        if name in csrs:
            v = csrs[name]
            extra = ""
            if name == "scause":
                interrupt = (v >> 63) & 1
                code = v & 0x7FFFFFFFFFFFFFFF
                cause_names = {
                    0: "Inst addr misaligned",
                    1: "Inst access fault",
                    2: "Illegal inst",
                    3: "Breakpoint",
                    4: "Load addr misaligned",
                    5: "Load access fault",
                    6: "Store addr misaligned",
                    7: "Store access fault",
                    8: "Ecall from U",
                    9: "Ecall from S",
                    12: "Inst page fault",
                    13: "Load page fault",
                    15: "Store page fault",
                }
                extra = f"  ({'IRQ' if interrupt else 'EXC'}: {cause_names.get(code, f'code {code}')})"
            if name == "satp":
                mode = (v >> 60) & 0xF
                asid = (v >> 44) & 0xFFFF
                ppn = v & 0x00000FFFFFFFFFFF
                extra = f"  (mode={mode} asid={asid} ppn=0x{ppn:x})"
            if name == "mstatus" or name == "sstatus":
                spp = (v >> 8) & 1
                spie = (v >> 5) & 1
                sie = (v >> 1) & 1
                extra = f"  (SPP={spp} SPIE={spie} SIE={sie})"
            print(f"  {name:>10} = 0x{v:016x}{extra}")

    # Decode instructions at crash site using page table walk
    print()
    print("=" * 72)
    print("  DECODED INSTRUCTIONS (from page table walk)")
    print("=" * 72)

    # Get unique PCs from the trace (only the ones where actual work happened)
    seen_pcs = []
    for _, ipc, _, changes in trace:
        if not seen_pcs or seen_pcs[-1] != ipc:
            seen_pcs.append(ipc)

    for va in seen_pcs:
        word, info = read_inst_at_va(cpu, va, satp)
        if word is None:
            print(f"  0x{va:016x}: UNMAPPED ({info})")
            continue
        is_c = (word & 0x3) != 0x3
        if is_c:
            asm = decode_rv_inst(word, va, is_compressed=True)
            print(f"  0x{va:016x}: {word & 0xFFFF:04x}      {asm}  [{info}]")
        else:
            asm = decode_rv_inst(word, va, is_compressed=False)
            print(f"  0x{va:016x}: {word:08x}  {asm}  [{info}]")

    # Show the decoded trace with register changes
    print()
    print("=" * 72)
    n = len(trace)
    show_n = min(n, 200)
    print(f"  INSTRUCTION TRACE ({n} entries, showing unique PCs)")
    print("=" * 72)

    for va in seen_pcs:
        word, _ = read_inst_at_va(cpu, va, satp)
        is_c = word is not None and (word & 0x3) != 0x3
        asm = "?"
        if word is not None:
            asm = decode_rv_inst(word, va, is_compressed=is_c)

        # Find register changes for this PC
        pc_changes = []
        stall_count = 0
        for cyc, ipc, _, changes in trace:
            if ipc == va:
                stall_count += 1
                if changes:
                    pc_changes.extend(changes)

        cstr = ""
        if pc_changes:
            cstr = " | " + ", ".join(
                f"{rn}:0x{old:x}->0x{new:x}" for _, rn, old, new in pc_changes
            )

        stall_info = f" ({stall_count} cyc)" if stall_count > 1 else ""
        print(f"  0x{va:016x}: {asm}{stall_info}{cstr}")

    # Detailed register state
    print()
    print("=" * 72)
    print("  REGISTER STATE AT TRAP")
    print("=" * 72)
    print(f"  Trap handler PC: 0x{trap_pc:016x}")
    for i in range(0, 32, 2):
        if i < len(trap_regs) and i + 1 < len(trap_regs):
            print(
                f"    {fmt_reg(i, trap_regs[i])}  "
                f"{fmt_reg(i + 1, trap_regs[i + 1])}"
            )

    # Memory inspection at critical addresses
    print()
    print("=" * 72)
    print("  MEMORY INSPECTION")
    print("=" * 72)

    stval = csrs.get("stval", 0)
    sepc = csrs.get("sepc", 0)

    # Try to read the faulting instruction
    if sepc:
        print(f"\n  Faulting instruction (sepc=0x{sepc:016x}):")
        word, info = read_inst_at_va(cpu, sepc, satp)
        if word is not None:
            is_c = (word & 0x3) != 0x3
            asm = decode_rv_inst(word, sepc, is_compressed=is_c)
            print(f"    {asm} (0x{word:08x})")
            print(f"    Page: {info}")

            # Extract load source register and offset
            opcode = word & 0x7F
            if opcode == 0x03:  # LOAD
                rs1 = (word >> 15) & 0x1F
                imm = (word >> 20) & 0xFFF
                if imm & 0x800:
                    imm |= ~0xFFF
                    imm = imm & 0xFFFFFFFFFFFFFFFF
                base_val = trap_regs[rs1] if rs1 < len(trap_regs) else 0
                eff_addr = (base_val + imm) & 0xFFFFFFFFFFFFFFFF
                print(
                    f"    Effective address: {REG_NAMES[rs1]}(0x{base_val:x}) + {imm & 0xFFFFFFFFFFFFFFFF:#x} = 0x{eff_addr:016x}"
                )

                # Try to translate the load address
                pa, pt_info = sv39_walk(cpu, eff_addr, satp)
                if pa is not None:
                    val = cpu.read_memory_u64(pa)
                    print(f"    Memory at PA 0x{pa:x}: 0x{val:016x}")
                else:
                    print(f"    Cannot translate: {pt_info}")
        else:
            print(f"    Cannot read: {info}")

    # Inspect the stval address
    if stval:
        print(f"\n  Faulting address (stval=0x{stval:016x}):")
        pa, info = sv39_walk(cpu, stval, satp)
        if pa is not None:
            val = cpu.read_memory_u64(pa)
            print(f"    PA=0x{pa:x}, value=0x{val:016x}")
        else:
            print(f"    {info}")

    # Inspect the address that loaded a5=0 at PC 0x3f89d07ffe
    # From previous trace: a5 was set to 0 at this PC
    # We need to decode the instruction to find what address it loaded from
    print(f"\n  Critical load analysis (a5 loaded as 0):")
    critical_pc = 0x3F89D07FFE
    word, info = read_inst_at_va(cpu, critical_pc, satp)
    if word is not None:
        is_c = (word & 0x3) != 0x3
        asm = decode_rv_inst(word, critical_pc, is_compressed=is_c)
        print(f"    Instruction at 0x{critical_pc:016x}: {asm}")
        print(f"    Encoding: 0x{word:08x}")

        opcode = word & 0x7F
        if opcode == 0x03:  # LOAD
            rs1 = (word >> 15) & 0x1F
            imm = (word >> 20) & 0xFFF
            if imm & 0x800:
                imm |= ~0xFFF
                imm = imm & 0xFFFFFFFFFFFFFFFF

            # Use entry regs to find the base value
            # a5 was 0x3f89d19878 right before this instruction (from trace analysis)
            # Actually, we need the register state at the time of this instruction
            # From the trace, before this inst: we can reconstruct
            print(f"    Base reg: {REG_NAMES[rs1]} (x{rs1})")
            print(f"    Offset: {imm:#x}")

            # Check what rs1 was at that point in the trace
            # Find the last register values before this instruction
            for cyc, ipc, regs, changes in trace:
                if ipc == critical_pc and changes:
                    # This is where a5 changed - find what rs1 was
                    # The regs here are AFTER the instruction, so we need prev regs
                    base_val = entry_regs[rs1] if rs1 < len(entry_regs) else 0
                    # Actually we need to track regs through the trace
                    break

            # Walk through trace to find register state at critical_pc
            current_regs = list(entry_regs)
            for cyc, ipc, new_regs, changes in trace:
                if ipc == critical_pc:
                    # current_regs has the state BEFORE this instruction
                    base_val = current_regs[rs1]
                    eff_addr = (base_val + imm) & 0xFFFFFFFFFFFFFFFF
                    print(f"    At cycle {cyc}: {REG_NAMES[rs1]}=0x{base_val:016x}")
                    print(f"    Effective VA: 0x{eff_addr:016x}")

                    pa, pt_info = sv39_walk(cpu, eff_addr, satp)
                    if pa is not None:
                        val = cpu.read_memory_u64(pa)
                        print(f"    Memory at PA 0x{pa:x}: 0x{val:016x}")
                        if val == 0:
                            print(
                                f"    *** Memory IS zero - this may be an "
                                f"uninitialized/unrelocated GOT entry ***"
                            )
                        else:
                            print(
                                f"    *** Memory is NON-ZERO now (0x{val:016x}) - "
                                f"load returned wrong value! ***"
                            )
                            print(
                                f"    *** This suggests a simulator bug "
                                f"(TLB/cache/store visibility issue) ***"
                            )
                    else:
                        print(f"    Cannot translate: {pt_info}")
                    break
                current_regs = list(new_regs)

    # Summary of all user sessions
    print()
    print("=" * 72)
    print(f"  ALL USER SESSIONS ({len(all_sessions)} total)")
    print("=" * 72)
    for i, sess in enumerate(all_sessions):
        n = len(sess.get("trace", []))
        epc = sess.get("entry_pc", 0)
        csrs_s = sess.get("csrs", {})
        sc = csrs_s.get("scause", 0)
        sv = csrs_s.get("stval", 0)
        print(
            f"  #{i}: entry=0x{epc:x}, " f"{n} inst, scause=0x{sc:x}, stval=0x{sv:016x}"
        )


if __name__ == "__main__":
    sys.exit(main())
