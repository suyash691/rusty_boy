# Game Boy (DMG) Cycle-Accurate Specification

Test-derived ground truth for the `rusty_boy` clean-room rebuild. Every value here is
extracted from the assembly sources of the test suites we run (gbmicrotest, mooneye-test-suite,
SameSuite, mealybug-tearoom-tests, age-test-roms). **The tests are the authority** — where a
reference emulator (e.g. SameBoy) deviates from a passing hardware test, the test wins.

Units: **1 T-cycle** = 1 dot = the master clock tick. **1 M-cycle** = 4 T-cycles. CPU ≈ 4.194 MHz.
A scanline is **456 dots** (114 M-cycles). A frame is 154 lines = 70224 dots.

---

## 1. CPU instruction bus schedules

Per-opcode M-cycle schedule (verbatim from mooneye `acceptance/*_timing.s` header comments).
`M=0` is the opcode fetch/decode; the **last** M-cycle overlaps the next instruction's opcode
fetch (fetch/execute overlap). "access" = a real bus read/write on that cycle.

| Instruction | M-cycles | Schedule |
|---|---|---|
| CALL nn | 6 | M1 read lo, M2 read hi, M3 internal, M4 **push hi**, M5 **push lo** |
| CALL cc (taken) | 6 | same as CALL nn |
| CALL cc (not taken) | 3 | M1 lo, M2 hi |
| JP nn | 4 | M1 lo, M2 hi, M3 internal |
| JP cc (taken) | 4 | same; (not taken) 3 |
| RET | 4 | M1 **pop lo**, M2 **pop hi**, M3 internal |
| RETI | 4 | same as RET; **IME enabled immediately** |
| RET cc (taken) | 5 | M1 internal, M2 pop lo, M3 pop hi, M4 internal; (not taken) 2 |
| POP rr | 3 | M1 lo, M2 hi (no internal delay) |
| PUSH rr | 4 | M1 internal, M2 **write hi**, M3 **write lo** |
| RST n | 4 | M1 internal, M2 push hi, M3 push lo |
| ADD SP, e | 4 | M1 read e, M2 internal, M3 internal |
| LD HL, SP+e | 3 | M1 read e, M2 internal |

Push order is **high byte first, low byte second**; pop is **low first, high second**.
Verified by `call_timing2`/`push_timing` aligning OAM-DMA end to a specific M-cycle.

The existing CPU instruction bodies (`src/cpu/opcodes.rs`, `alu.rs`, `cb_ops.rs`) already place
accesses on these cycles — blargg `cpu_instrs`/`instr_timing` and `mem_timing` pass 100%. They
are reused verbatim; only the timing *driver* changes.

---

## 2. Interrupts

**Dispatch = 5 M-cycles** then the ISR's first opcode is fetched:
1. 2 internal (wait) M-cycles
2. push PC high byte
3. push PC low byte
4. set PC = vector (0x40 + n*8)

(mooneye `intr_timing`: `nops 50 + trigger` ≡ `nops 61`; `51` ≡ `62`.)

- **EI**: enables IME with a **1-instruction delay** — the instruction after EI executes, then
  IME is active. (`ei_timing`, `ei_sequence`: `ei;nop;di`→interrupt fires; `ei;di`→none.)
- **DI**: on DMG disables IME **immediately** (`di_timing-GS`).
- **RETI**: enables IME **immediately** (a second pending interrupt dispatches right after).
- **`ie_push` quirk** (`acceptance/interrupts/ie_push.s`): during dispatch the two PC-push
  writes can land on IE ($FFFF) when SP≈$0000/$0001. The **high-byte push (first)** re-reads IE
  and can still **cancel/redirect** dispatch — if the pending bit is cleared, PC is forced to
  **$0000** instead of the vector (IF unchanged). The **low-byte push (second) is too late** to
  cancel; dispatch proceeds and IF is acknowledged.
- **HALT, IME=1**: services a pending interrupt immediately, same timing as a NOP sled.
- **HALT, IME=0, (IF&IE)≠0** → **HALT bug**: the byte after HALT is fetched but PC is **not
  incremented**, so that opcode executes twice. (gbmicrotest `halt_op_dupe`: `inc a` runs twice.)
- **HALT, IME=0, no pending**: continues immediately (no delay).
- **HALT preceded by EI**: behaves as IME=1 (normal service, no bug).

---

## 3. Timer

- **DIV** is a 16-bit counter incrementing every **T-cycle**; `FF04` exposes the upper 8 bits.
- **TIMA** increments on the **falling edge** of `(selected_DIV_bit AND TAC.enable)`. TAC freq
  select → DIV bit: `00`→bit 9, `01`→bit 3, `10`→bit 5, `11`→bit 7.
- **Overflow**: TIMA reads **$00 for 4 T-cycles**, then reloads TMA and sets IF.timer. No extra
  delay beyond those 4 T. (`timer/tima_reload`.)
- **Write-during-reload**: a CPU write to TIMA on the reload cycle is **ignored** (TMA wins);
  a write to TMA on the reload cycle uses the **new** value. (`tima_write_reloading`,
  `tma_write_reloading`.)
- **Glitches**: writing DIV (resets to 0) or TAC (changes selected bit) can create a falling
  edge → TIMA++. The selected bit must be sampled with the value **before** the write.

This model is implemented in `src/timer.rs` and passes **mooneye timer 13/13** — reused verbatim,
re-homed as a per-T-cycle `tick()` client of the central clock.

---

## 4. PPU

### Normal visible line (lines 1..143), 456 dots
- **Mode 2 (OAM scan)**: dots [0, 80). OAM locked.
- **Mode 3 (pixel transfer)**: dots [80, 80+len). OAM + VRAM locked. `len = 172 + (SCX % 8) +
  sprite_penalty + window_penalty`. Sprite penalty ≈ 6–11 dots/sprite (X-position dependent,
  see mooneye `intr_2_mode0_timing_sprites` table). Driven by the pixel FIFO + fetcher
  (`src/ppu/fifo.rs`), which already produces 172+SCX%8 and reads SCX/LCDC/BGP **live** during
  the fetch (required by mealybug mid-mode-3 register-change tests). Reused verbatim.
- **Mode 0 (HBlank)**: remainder to 456. Nothing locked.

### VBlank (lines 144..153)
- Mode 1. **LY=153 early wrap**: LY reads 153 for ~1 M-cycle then reads **0** (DMG drops one
  M-cycle earlier than CGB). (`line_153_*`.)
- VBlank entry (LY 143→144) also evaluates the **mode-2 OAM STAT source** (hardware quirk).

### LCD-enable quirk (software write of LCDC bit 7, 0→1)
The first scanline after a software enable is special — mooneye `lcdon_timing-GS` header:
*"line 0 starts with mode 0 and goes straight to mode 3; line 0 has different timings because
the PPU is late by 2 T-cycles; line 1 and 2 have normal timings."* **There is NO mode 2 on this
line.** gbmicrotest anchors (M-cycles after the LCDC write):
- mode 0 → mode 3 at **17 M-cyc** (`lcdon_to_stat3`: 16→$84, 17→$87).
- mode 3 → mode 0 at **60 M-cyc** (`lcdon_to_stat0`: 59→$87, 60→$84).
- OAM locks at **16**, unlocks at **59** (`lcdon_to_oam_unlock`: 15→$27, 16→$FF, 58→$FF, 59→$27).
- LY=1 at **110**; line-1 OAM at **111**.

Model: `LineKind::Enable` with **oam_len = 0** and a **2-T-cycle skew**. The anchors above must
then emerge as test results — they are NOT free tuning constants.

### Boot anchor (gbmicrotest `poweron_*`)
The machine boots with the LCD **already on** (the boot ROM enabled it) — this is a **normal
frame**, NOT the enable quirk. `poweron_stat` timeline (M-cyc from boot): line0 OAM 7–26, VRAM
27–69, HBlank 70–119, LY→1 at 120; line1 OAM 121–140, VRAM 141–183, HBlank 184–234 (114 M-cyc/
line). Clean-room init must place the PPU at the exact boot phase so STAT=$80, LY=$0A, DIV
internal=$ABCC hold at the `boot_hwio` sample point (see §7).

### STAT / interrupts
- STAT interrupt fires on the **rising edge** of the OR of: (LYC==LY AND STAT.6) | (mode0 AND
  STAT.3) | (mode1 AND STAT.4) | (mode2 AND STAT.5). Rising-edge only (STAT blocking).
- **No STAT interrupt while the LCD is off.**
- Access gating: **OAM locked in modes 2 and 3**; **VRAM locked in mode 3 only**. Locked reads
  return 0xFF; locked writes are dropped. **The lock the CPU observes is read/write-asymmetric and
  trails the STAT mode by ~1 dot** (GateBoy: lock = `XYMU_RENDERINGn`/scan signals, but a CPU READ
  latches late in the M-cycle while a WRITE commits early, so they straddle the mode edge). Modeled
  via a 1-dot-delayed mode snapshot `prev_mode` (`src/ppu/`):
  - VRAM read-locked = `mode==3 || prev_mode==3`; VRAM write-locked = `prev_mode==3`.
  - OAM read-locked = `mode>=2 || prev_mode>=2`; OAM write-locked =
    `prev_mode==3 || (prev_mode==2 && mode==2)` (the mode-0→2 and mode-2→3 transition dots are
    momentarily writable). Verified exact against gbmicrotest `oam/vram_read/write_l{0,1}_*`.

---

## 5. OAM DMA

- Write to `FF46` (value XX) copies XX00–XX9F to OAM.
- **Timing**: M0 = the write; M1 = OAM still accessible (fresh DMA); **transfer begins M2**;
  runs **160 M-cycles**, 1 byte/M-cycle. (`oam_dma_start`, `oam_dma_timing`.)
- During DMA the CPU can access **only HRAM** ($FF80–$FFFE); every other region reads 0xFF.
- A new write while a DMA runs restarts it (previous transfer keeps locking OAM until the new
  one's 160 cycles complete).
- `FF46` reads back the **last written value**, regardless of transfer state.

---

## 6. APU

- Frame sequencer clocked on the **falling edge of DIV bit 12** (single speed), 512 Hz, 8 steps:
  length on steps 0/2/4/6, sweep on 2/6, envelope on 7.
- Length-counter ranges: CH1/2/4 = 6-bit (`64 - n`); CH3 = 8-bit (`256 - n`).
- **Extra-length-clock quirk**: enabling the length counter (NRx4 bit 6 0→1) while the next
  sequencer step does NOT clock length decrements the counter once if non-zero.
- **Power-off (NR52 bit 7 → 0)**: resets channel registers; **DMG preserves length counters**;
  NRx1 length-load and NR52 remain writable while off.
- DAC: CH1/2/4 on iff `NRx2 & 0xF8 != 0`; CH3 via NR30 bit 7.
- DMG-relevant SameSuite APU tests: only `div_write_trigger` and `div_write_trigger_10` (the
  rest read CGB-only PCM12/PCM34 registers — behavior reference, not DMG pass vectors).

Channel logic is in `src/apu/` — reused, re-homed as a per-T-cycle client.

---

## 7. Boot / hardware init state (DMG ABC)

CPU registers at handoff (`boot_regs-dmgABC`):
`A=01 F=B0 B=00 C=13 D=00 E=D8 H=01 L=4D SP=FFFE PC=0100`.

IO map at handoff (`boot_hwio-dmgABCmgb`): P1=$CF, SB=$00, SC=$7E, **DIV visible=$AD (internal
counter ≈ $ABCC)**, TIMA=$00, TMA=$00, TAC=$F8, IF=$E1, NR10=$80 NR11=$BF NR12=$F3 NR14=$BF
NR21=$3F NR24=$BF NR30=$7F NR32=$9F NR34=$BF NR42=$00 NR43=$00 NR44=$BF NR50=$77 NR51=$F3
NR52=$F1, **LCDC=$91**, **STAT=$80**, SCY=$00 SCX=$00, **LY=$0A**, LYC=$00, BGP=$FC, WY=$00
WX=$00, IE=$00. OBP0/OBP1 uninitialized.

(DMG 0 variant differs: F=$00, B=$FF, E=$C1, H=$84, L=$03; DIV visible=$19, STAT=$83, LY=$01.
We target **DMG ABC**.)

---

## 8. DMG-applicable vs CGB-only test scope

- `-GS` suffix = "Game boy / Super game boy" DMG-family vectors (some skipped by our harness's
  current skip list, but DMG-correct).
- `-cgb*`, `-C`, `-S` suffixes and SameSuite APU channel tests = CGB/SGB-only; their numeric
  expected tables are not DMG pass criteria, but the *behaviors* they document are shared.
- This DMG build's realistic ceiling excludes CGB-only families (CGB HDMA, double-speed,
  PCM registers, CGB palette timing).
