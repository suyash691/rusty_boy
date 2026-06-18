# PPU T-Cycle Architecture Plan

## The SameBoy Model (from display.c source analysis)

### Key Facts (different from Pan Docs simplification)

1. **Mode 3 base duration = 167 dots** (not 172), plus SCX%8
2. The "172" from Pan Docs includes 5 dots of mode 2→3 transition overhead
3. Mode 2→3 transition: mode flag changes, then 5 dots of setup before fetcher starts
4. Fetcher runs at **1 T-cycle per state** (7 states: T1,T2,T1,T2,T1,T2,PUSH)
5. FIFO push only succeeds when **fifo_size == 0** (FIFO must be empty!)
6. FIFO starts pre-filled with 8 "junk" pixels
7. position_in_line starts at -16 (16 pixels discarded before visible output)
8. Each dot: `render_pixel_if_possible` + `advance_fetcher_state_machine`
9. Pixel rendering only occurs when BG FIFO is non-empty

### The Fetcher State Machine (7 states, 1 dot each)

```
GET_TILE_T1:     compute tilemap address
GET_TILE_T2:     read tile index from VRAM  
GET_DATA_LOW_T1: compute tile data address
GET_DATA_LOW_T2: read low byte from VRAM
GET_DATA_HIGH_T1: compute tile data address + 1
GET_DATA_HIGH_T2: read high byte from VRAM, advance window_tile_x
PUSH:            if fifo_size == 0: push 8 pixels, goto GET_TILE_T1
                 else: stay in PUSH (stall)
```

Each state takes exactly 1 dot. Total per tile = 7 dots (if push succeeds immediately).
If FIFO is not empty at PUSH, the fetcher stalls (stays in PUSH state) until it empties.

### How This Gives 167 Dots

- FIFO starts with 8 junk pixels (pre-pushed at mode 3 start)
- position_in_line = -16 (first 16 pixels discarded: 8 junk + 8 from SCX alignment)
- Fetcher starts immediately: GET_TILE_T1 on dot 0
- At dot 7 (PUSH): FIFO has 8 pixels (the junk). Push STALLS (fifo not empty).
- Meanwhile, rendering pops 1 pixel/dot from FIFO. After 8 dots, FIFO empties.
- Dot 8 onward: FIFO empty, PUSH succeeds, 8 new pixels enter.
- From dot 8: each tile takes 7 dots (fetcher) + stall until FIFO drains
  
Actually with position_in_line starting at -16 and SCX=0:
- Pixels are popped and discarded until position_in_line reaches 0 (visible)
- First visible pixel at position_in_line == 0, which is after 16+SCX%8 pops
- Total = initial_stall + 160 visible pixels = 167 + SCX%8

### The Rendering Loop (per dot)

```
1. Handle window activation
2. Handle sprite encounter  
3. render_pixel_if_possible():
   - if BG FIFO empty: do nothing
   - pop pixel from BG FIFO
   - if position_in_line < 0: discard (scrolling)
   - if position_in_line >= 0 and < 160: output to LCD
   - position_in_line++
4. advance_fetcher_state_machine()
5. if position_in_line == 160: end mode 3
```

### Mode 2→3 Transition (from SameBoy)

```
After OAM scan completes (80 dots):
  cycles_for_line = MODE2_LENGTH + 4  (= 84)
  Set STAT mode bits to 3
  Set mode_for_interrupt = 3
  Block VRAM/OAM
  STAT_update()
  SLEEP 3 dots
  Block CGB palettes
  SLEEP 2 dots
  → mode_3_start (total: 5 dots of overhead)
```

### Implementation Strategy

1. Replace fetcher with 7-state machine (1 dot per state)
2. Pre-fill FIFO with 8 junk pixels at mode 3 start
3. Set position_in_line to -16 (discards first 16 popped pixels)
4. Each dot: try render (pop if FIFO non-empty) + advance fetcher
5. PUSH only succeeds when fifo_size == 0
6. Mode 3 ends when position_in_line reaches 160
7. Add 5-dot overhead at mode 2→3 transition


## Problem Statement

We are stuck at 321/769 (242/513 gbmicrotest). The root cause is that our PPU
updates at M-cycle granularity (4 dots at a time) while the test ROMs observe
behavior at single-dot precision.

## Current Architecture

```
CPU M-cycle:
  tick_timer(3)           -- Timer runs T-cycle accurate (T0-T2)
  read/write              -- Bus access at T3
  tick_timer(1)           -- Timer runs T3
  tick_ppu_m_cycle()      -- PPU advances 4 dots IN BULK here
                          -- Interrupts collected from PPU → IF
                          -- DMA advances 1 byte
```

The PPU sees time in chunks of 4. Between those chunks, the CPU can read STAT,
read VRAM, etc. and always sees the state from the LAST ppu update. This means:
- Mode transitions can be up to 3 dots "late" from the CPU's perspective
- STAT reads during mode 3 don't reflect the current dot position
- VRAM/OAM blocking is 3 dots imprecise

## What Hardware Actually Does

On real hardware, the PPU runs on the SAME clock as the CPU. Every single dot:
- The PPU advances its state machine by 1 step
- The mode flag in STAT is updated in real time
- VRAM/OAM bus arbitration happens per-dot

The CPU accesses the bus at specific T-cycles within its M-cycle (T3 for reads).
At that exact T-cycle, it sees the PPU's current state (mode, LY, etc.).

## Required Changes

### 1. PPU ticks per dot inside the M-cycle

Instead of `ppu.update(4)` at the end, tick the PPU once per T-cycle alongside
the timer:

```rust
pub fn cycle_read(&mut self, addr: u16) -> u8 {
    self.tick_dot(); // T0: PPU dot 0
    self.tick_dot(); // T1: PPU dot 1  
    self.tick_dot(); // T2: PPU dot 2
    self.timer.update(3); // Timer ticks T0-T2
    let val = self.read_byte_bus(addr); // T3: bus read (sees PPU state AT this dot)
    self.tick_dot(); // T3: PPU dot 3
    self.timer.update(1); // Timer tick T3
    self.collect_interrupts();
    self.advance_dma();
    val
}
```

Where `tick_dot()` advances the PPU by exactly 1 dot and updates mode/LY/STAT
flags immediately.

### 2. PPU.update(1) - single dot step

The PPU state machine operates per-dot:

```
Mode 2: OAM scan
  - Dot 0-79: scanning OAM entries (2 dots per entry, 40 entries)
  - At dot 80: transition to mode 3

Mode 3: Pixel transfer  
  - Fetcher steps: Get Tile (2 dots), Get Data Low (2 dots), Get Data High (2 dots)
  - After Get Data High: attempt push (succeeds if FIFO ≤ 8)
  - Push: try every dot until success
  - Pixel output: pop 1 pixel per dot when FIFO > 0
  - First SCX%8 pixels are discarded
  - Ends when pixel_x == 160
  - Total: 172 + SCX%8 + sprite_penalties dots

Mode 0: HBlank
  - Simply counts remaining dots until scanline reaches 456

Mode 1: VBlank
  - 10 scanlines × 456 dots
```

### 3. Fetcher timing (correct model)

Per Pan Docs, the fetcher has these steps at 2 dots each:
1. Get Tile (2 dots)
2. Get Tile Data Low (2 dots)  
3. Get Tile Data High (2 dots) - AND attempts push
4. Sleep (2 dots) - AND attempts push
5. Push - attempts every dot until success

Total: 8 dots minimum per tile. But Push can succeed at step 3 or 4 if FIFO
has space. The FIRST tile always pushes at step 3 (FIFO is empty), taking only
6 dots effective. Subsequent tiles may stall at Push if FIFO is full (8 pixels
already queued from fast fetch vs slow pixel output).

BUT: the pixel output runs at 1 pixel/dot. The fetcher produces 8 pixels per
8 dots (if stalling) or 8 pixels per 6 dots (if push succeeds early). So:
- 6 dots/tile: fetcher is faster than output, FIFO builds up, Push stalls
- 8 dots/tile (with stall): fetcher matches output exactly

The steady state is 8 dots/tile because once FIFO has 8 pixels, Push must wait
for space (FIFO ≤ 8 means it must drain to ≤ 8 before next push).

For the FIRST tile: 6 dots (push succeeds immediately into empty FIFO).
For the SECOND tile: starts at dot 6. Takes 6 dots for data, push at dot 12
succeeds because FIFO drained from 8 to 2 by then (6 pixels shifted out).

So: first pixel output at dot 7 (after first push at dot 6 + 1 dot to start shifting).
But spec says first pixel at dot 12. The discrepancy is because the OUTPUT
doesn't start until the FIFO has been primed with enough pixels. The spec says
"8 pixels are required for the Pixel Rendering operation to take place" — so
output only begins when FIFO reaches 8 pixels.

First push at dot 6 puts 8 pixels in FIFO. Output starts at dot 7 (FIFO has 8).
That gives 7 + 160 = 167. Still 5 short of 172.

The missing 5: the first tile fetch is SPECIAL. It's actually:
- 2 dots: fetcher reset/startup  
- 6 dots: first tile fetch
- 6 dots: second tile fetch (first "real" tile data, discards from first)
Total setup: 12 dots before first pixel output. Then 160 pixels = 172 total.

The "discarded" second fetch means: the first fetch's 8 pixels are ALL thrown
away (the SCX%8 discard plus the remaining pixels that get replaced by the
true first visible tile). This is the "dummy" tile.

### 4. Implementation plan

**Phase 1: Change PPU.update to single-dot**
- Change `pub fn update(&mut self, cycles: u32)` to iterate per dot internally
  but still be called with 4 (or 1). Key: mode transitions happen at exact dots.

**Phase 2: Change tick_ppu_m_cycle to tick 4 individual dots**
- Instead of `self.ppu.update(4)`, do 4× `self.ppu.update(1)`
- This alone gives dot-accurate mode transitions visible to the CPU

**Phase 3: Interleave PPU dots with CPU T-cycles**
- Move PPU tick into the `tick_timer` path or parallel to it
- The CPU bus read/write at T3 sees the PPU state after 3 dots of advance

**Phase 4: Fix fetcher to match spec**
- First tile takes 6 dots (push succeeds into empty FIFO)
- Output starts after 12 dots (two tile fetches complete)
- Subsequent tiles take 6-8 dots depending on FIFO fullness

### 5. What this fixes

- ~68 tests that are "off by 1" (mode 3→0 transition dot precision)
- HBlank interrupt timing (correct to the dot)
- VRAM/OAM access blocking (correct to the dot)  
- STAT register reads (reflect exact current state)
- VBlank interrupt timing (cumulative scanline accuracy)

### 6. Risk assessment

- The timer is ALREADY T-cycle accurate (we call timer.update(3) then timer.update(1))
- PPU just needs the same treatment
- APU can stay at M-cycle resolution (its tests don't need dot precision)
- DMA stays at M-cycle resolution (1 byte per M-cycle is correct)
- The main risk is performance: 4× more PPU calls per M-cycle

### 7. Starting point

The simplest change with maximum impact:
**Replace `self.ppu.update(ppu_cycles)` with a loop of `self.ppu.update(1)`**

This single change gives us dot-accurate mode transitions without restructuring
the CPU loop. The PPU already handles per-dot mode 3 (it loops internally).
We just need modes 0, 1, 2 to also advance per-dot instead of in bulk.
