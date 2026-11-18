# PlanCraft scheduling mechanics

Values read from the original program (WINPROJ.EXE and its English UI resource file) during the
study. Each line says where it came from. "estimated" means it was looked for in the code and not
found, so a sensible value was chosen instead.

## Calendar and time units

| Rule | Value | Source |
|---|---|---|
| Minutes per working day | 480 | new-project defaults initializer, function 0x1401ee6a8 (`+0x40 = 0x1e0`) |
| Minutes per working week | 2400 | same function (`+0x44 = 0x960`) |
| Working days per month | 20 | same function (`+0x48` low word `0x14`) |
| Default duration unit | days (unit code 7) | same function (`+0x48` high word `7`); unit codes minute 3, hour 5, day 7, week 9, month 11 |
| Days per calendar week | 7 | assertion text `CDR_DAYS_PER_WEEK` in CalendarReader::GetEffectiveWeekExtents, function 0x140202804 |
| Standard calendar working days | Mon–Fri (pattern `0111110`, Sunday first) | UI resource file, offset 0x35c440 |
| Standard calendar hours | 08:00–12:00 and 13:00–17:00 | UI resource file, next to the pattern above |
| 24 Hours calendar | every day 00:00–24:00 (pattern `1111111`) | UI resource file |
| Night Shift calendar | Mon 23:00–24:00; Tue–Fri 00:00–03:00, 04:00–08:00, 23:00–24:00; Sat 00:00–03:00, 04:00–08:00; Sun off (pattern `0111111`) | UI resource file |
| Default start time | 08:00 | follows from the Standard calendar's first shift (estimated as the default start time setting) |
| Default finish time | 17:00 | follows from the Standard calendar's last shift (estimated) |

Conversions used everywhere: 1 hour = 60 min, 1 day = minutes-per-day, 1 week = minutes-per-week,
1 month = days-per-month × minutes-per-day. Elapsed units (`ed`, `ew`...) count wall-clock time:
day 1440, week 10080, month 43200 minutes.

## Scheduling types (order as stored by the original)

* Link types: Finish-to-Start (FS), Start-to-Start (SS), Finish-to-Finish (FF), Start-to-Finish (SF)
  — UI resource file offset 0x4057e0. File codes when exported: FF 0, FS 1, SF 2, SS 3.
* Constraint types, codes 0–7: ASAP, ALAP, MSO, MFO, SNET, SNLT, FNET, FNLT — abbreviation list in
  the UI resource file (offset ≈0x143e00 as UTF-16 text "ASAP|ALAP|MSO|MFO|SNET|SNLT|FNET|FNLT").
* Task types, codes 0–2: Fixed Units, Fixed Duration, Fixed Work — same resource block.
* Resource types: Work, Material, Cost — same resource block.
* Priority scale: 0–1000, default 500 (estimated; the UI lists Lowest…Highest levels).

## Scheduling rules implemented

1. Forward pass in dependency order. A task starts at the project start, or later when a link or
   constraint demands it:
   * FS: start ≥ predecessor finish + lag; SS: start ≥ predecessor start + lag;
     FF: finish ≥ predecessor finish + lag; SF: finish ≥ predecessor start + lag.
   * Lags are working time on the project calendar (negative lags = leads).
   * A zero-length milestone after an FS link sits at the predecessor's finish moment.
   * SNET/FNET push later; MSO/MFO pin; SNLT/FNLT pull earlier (constraints win over links, the
     "honor constraints" setting found as `HonorConstraints` in the object model table).
   * Typing a start date on an automatic task adds a Start No Earlier Than constraint; typing a
     finish adds Finish No Earlier Than (estimated: standard behaviour, not traced in code).
2. Backward pass from the project finish gives late start/finish; deadlines and FNLT/SNLT limit
   late finish.
3. Total slack = min(late start − early start, late finish − early finish) in working minutes.
   A task is critical when total slack ≤ critical slack (0 days, estimated; field `CRITICALFLOAT`
   exists, its default was not traced).
4. ALAP tasks are placed at their late dates.
5. Summary tasks roll up: earliest start, latest finish, sums of work and cost,
   duration-weighted % complete. Links on a summary apply to all of its subtasks.
6. Work = duration × assignment units. Task type decides what changes:
   * Fixed Units: changing duration changes work; changing units or work changes duration.
   * Fixed Work: changing duration changes units; changing units changes duration.
   * Fixed Duration: changing units or work keeps duration.
   * Effort-driven tasks keep total work when resources are added (off by default, estimated).
7. Cost = work hours × standard rate + cost per use (+ material units × rate, + cost resource
   amounts) + fixed cost.
8. Leveling (estimated: the original's algorithm was not decompiled): delay the lower-priority,
   later task of each overallocation to the finish of the conflicting task until no resource
   exceeds its maximum units.
9. Baselines 0–10 save start, finish, duration, work and cost (field names BASELINE…BASELINE10
   found in the file's field table, functions 0x1401ba830 / 0x1401bd990).

## Reading .mpp files (src/mpp.rs)

The layout below was found by having the installed original save known plans as .mpp, and
comparing every record with its own XML export of the same file (tools: `examples/mpp*.rs`,
test files in `mpptest/`; `cargo run --example mppcheck -- mpptest/*.mpp` matches 1072 of 1072
values across four plans, one built directly in the original with deleted and re-added rows). Only the 2010-and-later layout (`   114` folder) is read; older
files get a message asking to re-save them.

- Container: OLE compound file. Tables `TBkndTask`, `TBkndRsc`, `TBkndAssn`, `TBkndCons`, each
  with `FixedMeta`/`FixedData`, `Fixed2Meta`/`Fixed2Data` and `VarMeta`/`Var2Data`
  (magic 0xFADFADBA). A meta item's u32 at +4 is its record's offset in the data stream;
  byte 0 bit 0x02 marks a deleted record, whose stale data is still there.
- VarMeta entries are 12 bytes (uid u32, offset u32, type u16, flags u16); Var2Data holds a
  u32 size then the bytes. Task name = type 14 (UTF-16), notes = type 15 (RTF), WBS = 16.
  Resource name = 1, initials = 2, group = 3, material label = 299.
- Dates: u16 tenths of a minute since midnight + u16 days since 1983-12-31.
- Task record: ID (row) u32 @0, unique ID u32 @4, constraint type u16 @64, constraint date @66, priority
  u16 @78, duration u32 @84 (tenths of a minute), % complete u16 @92, actual start @96, start
  @104, finish @108, fixed cost f64 @48, duration units u16 @164, outline level u16 @172,
  deadline @182. Flags: milestone = FixedMeta item byte 10 bit 0x02; manual = Fixed2Meta item
  byte 8 bit 0x80; active = same byte bit 0x40. uid 0 is the project summary.
- Link record (20 bytes): predecessor uid @4, successor uid @8, type u16 @12 (0 FF, 1 FS, 2 SF,
  3 SS), lag u32 @14 (tenths of a minute), lag units u16 @18.
- Resource record: ID @0, unique ID @4, max units f64 @8 (percent x 100), standard rate f64 @16 (per hour).
  Material = FixedMeta byte 12 bit 0x04 clear; cost = Fixed2Meta byte 8 bit 0x10 set.
- Assignment record: uid @0, task uid @4, resource uid @8, units f64 @12 (percent x 100), work
  f64 @20 (minutes x 1000), cost f64 @68 (cents).
- Not read yet: calendars (the Standard calendar is used), baselines, custom fields,
  password-protected files.
