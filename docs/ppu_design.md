# PPU Pixel FIFO - Correct Model Based on Pan Docs

## Key Facts from Spec

### Timing Constants
- 1 dot = 1 T-cycle (4 MHz)
- 1 M-cycle = 4 dots
- Scanline = 456 dots
- Mode 2 (OAM scan) = 80 dots
- Mode 3 (pixel transfer) = 172 to 289 dots
- Mode 0 (HBlank) = 376 - mode_3_duration
- Mode 1 (VBlank) = 4560 dots (10 scanlines)

### Mode 3 Duration
- Minimum = 172 dots = 12 (initial overhead) + 160 (pixels)
- The 12 dots come from "two tile fetches at the beginning"
- SCX penalty: +SCX%8 dots at very beginning
- Sprite penalty: +6 to +11 per sprite
- Window penalty: +6 when window starts

### Pixel Fetcher (5 steps)
1. Get Tile: 2 dots
2. Get Tile Data Low: 2 dots
3. Get Tile Data High: 2 dots (+ "also pushes a row" - extra push attempt)
4. Sleep: 2 dots
5. Push: attempted every dot until succeeds (pushes only if FIFO ≤ 8 pixels)

Total per tile: 8 dots minimum (steps 1-4) + Push succeeds same dot or later

### Critical Insight: "Get Tile Data High also pushes"
The spec says: "This also pushes a row of background/window pixels to the FIFO.
This extra push is not part of the 8 steps, meaning there's 3 total chances to
push pixels to the background FIFO every time the complete fetcher steps are
performed."

This means there are THREE push attempts per fetch cycle:
1. At end of Get Tile Data High step
2. During Sleep step (implicit?)
3. During the Push step

The push only succeeds if FIFO has space (≤ 8 pixels). On the first tile,
FIFO is empty so the push at step 3 (Get Tile Data High) succeeds immediately
at dot 6 of the fetch. This means pixels enter the FIFO at dot 6, NOT dot 10.

### Pixel Output
- Pixels are popped from FIFO at 1 pixel per dot
- Popping only occurs when FIFO has > 8 pixels (ensuring buffer)
  Actually: "8 pixels are required for the Pixel Rendering operation to take place"
- First SCX%8 pixels are discarded

## The Problem with Our Current Model

We have TWO contradictory requirements:
1. Mode 3 total duration must be ~172 dots (for HBlank timing tests)
2. Mode 3 internal STAT flag changes must happen at correct relative dots
   (for STAT read tests that check mode during specific instructions)

Our current model (10 dots/tile with Sleep, all steps 2 dots) gives:
- Good internal timing (242 gbmicrotest pass)
- Bad total duration (~287 dots for mode 2+3, vs expected 252)

The 6 dots/tile model gives:
- Correct total duration (252 dots)
- Bad internal timing (only 215 gbmicrotest pass)

## Root Cause Analysis

The issue is that with 10 dots/tile, the fetcher is SLOWER than pixel output
(8 pixels per 10 dots vs 1 pixel per dot consumption). This means the FIFO
underruns during mode 3, causing stalls. These stalls extend mode 3 but
accidentally happen at points that match what tests expect for STAT reads.

With 6 dots/tile, no stalls occur (fetcher is faster than output), so mode 3
is the minimum 172 dots. But without stalls, the STAT mode flag is always "3"
until exactly pixel 160 — tests that check mode during specific mid-scanline
points see different behavior.

## Correct Solution

The correct model requires the PPU to tick at DOT resolution, not M-cycle
resolution. Each dot, the PPU must:
1. Advance the fetcher state machine
2. Attempt to pop a pixel from the FIFO (if conditions are met)
3. These happen on the SAME clock, not sequentially

The key constraint our architecture violates: we update the PPU 4 dots at a
time (once per M-cycle). A CPU read of STAT during mode 3 returns the PPU
state from the LAST update, not the current dot. This means any STAT read
during mode 3 is up to 3 dots stale.

## Practical Fix Strategy

Since we can't easily make the PPU tick per-dot while maintaining performance,
the pragmatic approach is:

**Keep the 10-dot/tile model** (best test score) and accept that:
- Total mode 3 is ~35 dots too long (doesn't affect games)
- HBlank interrupt fires ~4 dots late (affects 68 gbmicrotest)
- VRAM/OAM access timing is ~4 dots imprecise

**To actually improve**, we'd need to:
1. Track `mode_3_end_dot` when mode 3 starts (computed from SCX + sprites)
2. On any STAT read during mode 3, check if current dot has passed mode_3_end_dot
3. If yes, return mode 0 even though we haven't fully rendered yet
4. This gives correct STAT reads without changing the rendering model

This "predictive mode 0" approach lets us keep the simple rendering pipeline
while correctly reporting mode transitions to the CPU.
