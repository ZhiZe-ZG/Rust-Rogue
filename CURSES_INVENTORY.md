# Curses API Inventory

This document lists every ncurses function called across C and Rust source,
grouped by purpose, with call sites and suggested crossterm equivalents.

---

## 1. Initialization & Teardown

| Function | C call sites | Rust call sites | crossterm equivalent |
|---|---|---|---|
| `initscr()` | `main.c:116`, `main.c:97` | `save.rs:270` | `terminal::enable_raw_mode()` + `execute!(stdout, EnterAlternateScreen)` |
| `endwin()` | `main.c:124`, `main.c:181`, `main.c:217`, `main.c:336`, `main.c:356` | `monsters.rs:367`, `save.rs:218`, `save.rs:274`, `save.rs:280`, `save.rs:299` | `terminal::disable_raw_mode()` + `execute!(stdout, LeaveAlternateScreen)` |
| `newwin(lines, cols, y, x)` | `main.c:141`, `things.c:503` | `save.rs:286` | Allocate an off-screen `Vec<Vec<char>>` buffer |
| `delwin(win)` | `rip.c:70`, `rip.c:71`, `rip.c:73`, `things.c:532` | — | Drop the buffer |
| `subwin(win, …)` | `things.c:504` | — | Slice a sub-region of the off-screen buffer |
| `keypad(stdscr, 1)` | `mach_dep.c:156`, `main.c:228`, `main.c:371` | `save.rs:271` | No equivalent needed; crossterm reads escape sequences natively |
| `idlok(win, flag)` | `main.c:142`, `main.c:143` | — | No equivalent (crossterm does not need this) |
| `clearok(win, flag)` | — | `save.rs:296`, `save.rs:307` | Not needed once using crossterm |

---

## 2. Terminal Mode

| Function | C call sites | Rust call sites | crossterm equivalent |
|---|---|---|---|
| `raw()` | `mach_dep.c:154`, `main.c:226`, `main.c:370`, `mdport.c:1027`, `mdport.c:1246` | — | `terminal::enable_raw_mode()` |
| `noecho()` | `mach_dep.c:155`, `main.c:227`, `main.c:369` | — | Included in `enable_raw_mode()`; or disable with `event::DisableMouseCapture` |
| `nocbreak()` | `mdport.c:1026`, `mdport.c:1245` | — | `terminal::disable_raw_mode()` |
| `halfdelay(tenths)` | `mdport.c:1132` | — | `event::poll(Duration)` |
| `flushinp()` | `mach_dep.c:456` | — | Drain `event::read()` in a loop while `event::poll(Duration::ZERO)` |
| `typeahead(fd)` | `mach_dep.c:~450` (referenced) | — | Not needed |
| `baudrate()` | `main.c:253` | — | Not applicable; remove the baud-rate optimization |

---

## 3. Screen Refresh

| Function | C call sites | Rust call sites | crossterm equivalent |
|---|---|---|---|
| `refresh()` | `command.c:61`, `io.c:105`, `main.c:180`, `main.c:302`, `main.c:312`, `main.c:355`, `rip.c:254`, `rip.c:302`, `rip.c:368`, `things.c:502` | `player.rs:367`, `save.rs:152`, `sticks.rs:405`, `weapons.rs:253` | `stdout().flush()` after writing all output |
| `wrefresh(win)` | `command.c:291`, `io.c:273`, `io.c:276` (stdscr), `main.c:231`, `options.c:115`, `options.c:183`, `options.c:260`, `options.c:309`, `options.c:337`, `things.c:524`, `things.c:530`, `things.c:539`, `command.c:587`, `command.c:595` | — | Flush after writing the off-screen buffer |
| `touchwin(win)` | `command.c:594`, `io.c:271`, `io.c:276`, `options.c:118`, `things.c:523`, `things.c:533`, `things.c:543` | — | Mark the buffer as dirty (no-op in crossterm model) |

---

## 4. Cursor Movement

| Function | C call sites | Rust call sites | crossterm equivalent |
|---|---|---|---|
| `move(y, x)` | `command.c:547`, `command.c:59`, `daemons.c:221`, `daemons.c:268`, `fight.c:359`, `io.c:222`, `io.c:230`, `io.c:239`, `main.c:234`, `main.c:301`, `main.c:308`, `main.c:311`, `main.c:354`, `misc.c:117`, `misc.c:233`, `passages.c:339`, `rip.c:238`, `rip.c:248`, `rip.c:253` | `player.rs:262`, `player.rs:273`, `player.rs:333`, `potions.rs:563`, `potions.rs:601` | `execute!(stdout, MoveTo(x as u16, y as u16))` |
| `wmove(win, y, x)` | `command.c:578`, `command.c:585`, `io.c:269`, `io.c:272`, `options.c:89`, `options.c:99`, `options.c:105`, `options.c:113`, `options.c:182`, `options.c:205`, `options.c:209`, `options.c:282`, `options.c:336`, `options.c:364`, `things.c:507`, `things.c:511`, `things.c:537` | `potions.rs:415`, `potions.rs:427`, `potions.rs:450`, `scrolls.rs:450` | `MoveTo` on the off-screen buffer |
| `getyx(win, y, x)` | `io.c:201`, `main.c:215`, `main.c:232`, `options.c:178`, `options.c:259`, `options.c:332`, `things.c:552` | — | `cursor::position()` → `(col, row)` |
| `mvcur(ly, lx, y, x)` | `main.c:216`, `main.c:233`, `main.c:335` | `save.rs:216` | `execute!(stdout, MoveTo(x as u16, y as u16))` |
| `mvwin(win, y, x)` | `things.c:519`, `things.c:521` | — | Reposition the off-screen buffer's origin |

---

## 5. Output: Character

| Function | C call sites | Rust call sites | crossterm equivalent |
|---|---|---|---|
| `addch(ch)` | `chase.c:101`, `chase.c:105`, `daemons.c:224`, `daemons.c:226`, `daemons.c:230`, `daemons.c:272`, `daemons.c:274`, `daemons.c:279`, `misc.c:123`, `passages.c:343`, `passages.c:347`, `rip.c:287-301` (multi) | `monsters.rs:345`, `monsters.rs:347`, `player.rs:271`, `player.rs:280`, `player.rs:283`, `player.rs:286`, `player.rs:337`, `player.rs:342`, `player.rs:347`, `potions.rs:567`, `potions.rs:574`, `potions.rs:576` | `print!("{}", ch)` at current cursor position |
| `mvaddch(y, x, ch)` | `chase.c:88`, `daemons.c:101`, `daemons.c:213`, `daemons.c:254`, `daemons.c:260`, `fight.c:95`, `fight.c:161`, `fight.c:612`, `io.c:~100` (via msg), `main.c:179`, `misc.c:178`, `new_level.c:95`, `pack.c:185`, `pack.c:454`, `pack.c:46`, `passages.c:~340`, `rip.c:243`, `rip.c:245`, `rip.c:246`, `rip.c:251`, `rip.c:301`, `rip.c:305`, `wizard.c:204`, `wizard.c:217` | `player.rs:390`, `potions.rs:549`, `scrolls.rs:437`, `sticks.rs:404`, `trap.rs:159`, `weapons.rs:243`, `weapons.rs:252`, `weapons.rs:273` | `execute!(stdout, MoveTo(x,y))` + `print!("{}", ch)` |
| `waddch(win, ch)` | `options.c:84`, `options.c:211`, `options.c:275` | `potions.rs:416`, `potions.rs:428`, `scrolls.rs:451` | Write `ch` into off-screen buffer at current position |
| `mvwaddch(win, y, x, ch)` | `state.c:764` | — | Write `ch` into off-screen buffer at `(y,x)` |
| `inch()` | `fight.c:360`, `misc.c:122`, `misc.c:234` | `player.rs:233`, `potions.rs:602` | Read character from a virtual screen buffer at current cursor |
| `mvinch(y, x)` | `chase.c:243` | — | Read character from virtual screen buffer at `(y,x)` |
| `mvwinch(win, y, x)` | `state.c:735`, `things.c:509` | — | Read character from off-screen buffer at `(y,x)` |

---

## 6. Output: String & Formatted

| Function | C call sites | Rust call sites | crossterm equivalent |
|---|---|---|---|
| `addstr(s)` | `rip.c:240`, `rip.c:249`, `rip.c:287-301` (victory screen lines) | `save.rs:151` | `print!("{}", s)` |
| `mvaddstr(y, x, s)` | `io.c:100`, `io.c:77`, `main.c:179`, `rip.c:241`, `rip.c:245`, `rip.c:246`, `rip.c:251`, `rip.c:301`, `rip.c:305` | — | `execute!(stdout, MoveTo(x,y))` + `print!("{}", s)` |
| `waddstr(win, s)` | `command.c:580`, `command.c:581`, `command.c:586`, `io.c:270`, `options.c:114`, `options.c:141`, `options.c:152`, `options.c:163`, `options.c:179`, `options.c:206`, `options.c:210`, `options.c:292`, `options.c:302`, `options.c:333`, `options.c:365`, `things.c:512`, `things.c:538` | — | Write into off-screen buffer |
| `printw(fmt, …)` | `rip.c:231`, `rip.c:232`, `rip.c:364`, `rip.c:367` | — | `print!("{}", ...)` |
| `mvprintw(y, x, fmt, …)` | `main.c:300`, `rip.c:228` | — | `execute!(stdout, MoveTo(x,y))` + `print!("{}", ...)` |
| `wprintw(win, fmt, …)` | `options.c:130` | — | `write!` into off-screen buffer |

---

## 7. Screen Clearing

| Function | C call sites | Rust call sites | crossterm equivalent |
|---|---|---|---|
| `clear()` | `new_level.c:42`, `rip.c:224`, `rip.c:285`, `rip.c:304` | — | `execute!(stdout, Clear(ClearType::All))` |
| `clrtoeol()` | `io.c:101`, `io.c:238`, `main.c:309` | — | `execute!(stdout, Clear(ClearType::UntilNewLine))` |
| `wclear(win)` | `command.c:573`, `options.c:76`, `things.c:482`, `things.c:542` | `potions.rs:410`, `scrolls.rs:445` | Clear off-screen buffer |
| `wclrtoeol(win)` | `options.c:265` | — | Clear off-screen buffer from cursor to end of line |

---

## 8. Attributes / Highlighting

| Function | C call sites | Rust call sites | crossterm equivalent |
|---|---|---|---|
| `standout()` | `chase.c:104`, `daemons.c:229`, `daemons.c:278`, `passages.c:346`, `rip.c:286` | `monsters.rs:343`, `player.rs:279`, `player.rs:341`, `potions.rs:571` | `execute!(stdout, SetAttribute(Attribute::Reverse))` |
| `standend()` | `chase.c:106`, `daemons.c:231`, `daemons.c:280`, `passages.c:348`, `rip.c:297` | `monsters.rs:349`, `player.rs:281`, `player.rs:343`, `potions.rs:579` | `execute!(stdout, SetAttribute(Attribute::Reset))` |

---

## 9. Input

| Function | C call sites | Rust call sites | crossterm equivalent |
|---|---|---|---|
| `getch()` | `mdport.c:1021` | — | `event::read()` → `Event::Key(KeyEvent { … })` |

---

## Rust Call Sites by File

### `save.rs`
| Line | Call |
|---|---|
| 151 | `addstr(c"Yes\n".as_ptr())` |
| 152 | `refresh()` |
| 216 | `mvcur(0, cols - 1, lines - 1, 0)` |
| 218 | `endwin()` |
| 270 | `initscr()` |
| 271 | `keypad(stdscr, 1)` |
| 274 | `endwin()` |
| 280 | `endwin()` |
| 286 | `hw = newwin(LINES, COLS, 0, 0)` |
| 296 | `clearok(stdscr, 1)` |
| 299 | `endwin()` |
| 307 | `clearok(curscr, 1)` |

### `player.rs`
| Line | Call |
|---|---|
| 233 | `inch() as u8 as c_char` |
| 262 | `r#move(y, x0)` |
| 271 | `addch(ch as c_uint)` |
| 273 | `r#move(y, x + 1)` |
| 279 | `standout()` |
| 280 | `addch((*thing_t(tp)).t_disguise as c_uint)` |
| 281 | `standend()` |
| 283 | `addch(ch as c_uint)` |
| 286 | `addch((*thing_t(tp)).t_disguise as c_uint)` |
| 333 | `r#move(y, x)` |
| 337 | `addch(SPACE as c_uint)` |
| 341 | `standout()` |
| 342 | `addch(ch as c_uint)` |
| 343 | `standend()` |
| 347 | `addch(out as c_uint)` |
| 367 | `refresh()` |
| 390 | `mvaddch(hero.y, hero.x, floor_at() as c_uint)` |

### `potions.rs`
| Line | Call |
|---|---|
| 410 | `wclear(hw)` |
| 415 | `wmove(hw, (*thing_o(tp)).o_pos.y, (*thing_o(tp)).o_pos.x)` |
| 416 | `waddch(hw, MAGIC as c_uint)` |
| 427 | `wmove(hw, (*thing_t(mp)).t_pos.y, (*thing_t(mp)).t_pos.x)` |
| 428 | `waddch(hw, MAGIC as c_uint)` |
| 549 | `mvaddch((*thing_t(mp)).t_pos.y, (*thing_t(mp)).t_pos.x, (*thing_t(mp)).t_disguise as c_uint)` |
| 563 | `r#move((*thing_t(mp)).t_pos.y, (*thing_t(mp)).t_pos.x)` |
| 567 | `addch((*thing_t(mp)).t_oldch as c_uint)` |
| 571 | `standout()` |
| 574 | `addch((*thing_t(mp)).t_type as c_uint)` |
| 576 | `addch((rnd(26) + 'A' as c_int) as c_uint)` |
| 579 | `standend()` |
| 601 | `r#move(stairs.y, stairs.x)` |
| 602 | `if inch() == STAIRS` |

### `monsters.rs`
| Line | Call |
|---|---|
| 343 | `standout()` |
| 345 | `addch((*thing_t(tp)).t_type as c_uint)` |
| 347 | `addch((rnd(26) + 'A' as c_int) as c_uint)` |
| 349 | `standend()` |
| 367 | `endwin()` |

### `scrolls.rs`
| Line | Call |
|---|---|
| 437 | `mvaddch(y, x, ch as c_uint)` |
| 445 | `wclear(hw)` |
| 450 | `wmove(hw, (*thing_o(it)).o_pos.y, (*thing_o(it)).o_pos.x)` |
| 451 | `waddch(hw, FOOD as c_uint)` |

### `weapons.rs`
| Line | Call |
|---|---|
| 243 | `mvaddch((*o).o_pos.y, (*o).o_pos.x, ch as c_uint)` |
| 252 | `mvaddch((*o).o_pos.y, (*o).o_pos.x, (*o).o_type as c_uint)` |
| 253 | `refresh()` |
| 273 | `mvaddch(FALL_POS.y, FALL_POS.x, (*thing_o(obj)).o_type as c_uint)` |

### `sticks.rs`
| Line | Call |
|---|---|
| 404 | `mvaddch(pos.y, pos.x, '/' as c_char)` |
| 405 | `refresh()` |

### `trap.rs`
| Line | Call |
|---|---|
| 159 | `mvaddch((*tc).y, (*tc).x, TRAP as c_uint)` |

---

## Summary Table

| Category | Unique functions | Total call sites (C + Rust) |
|---|---|---|
| Init / Teardown | 8 | ~22 |
| Terminal mode | 7 | ~15 |
| Screen refresh | 3 | ~30 |
| Cursor movement | 5 | ~60 |
| Character output | 7 | ~80 |
| String / formatted output | 6 | ~40 |
| Screen clearing | 4 | ~15 |
| Attributes | 2 | ~18 |
| Input | 1 | 1 |
| **Total** | **43** | **~281** |

---

## Migration Notes

### Globals that must be replaced

| ncurses global | Usage | crossterm equivalent |
|---|---|---|
| `stdscr` | Default window pointer, passed to `keypad`, etc. | Not needed |
| `curscr` | Physical screen; used in `wrefresh(curscr)`, `getyx(curscr, …)` in `main.c` | Not needed; crossterm targets physical terminal directly |
| `hw` | Off-screen window for menus/help (created with `newwin`) | `Vec<Vec<char>>` scratch buffer |
| `LINES` / `COLS` | Terminal dimensions | `terminal::size()` → `(cols, rows)` |

### Windowed rendering (`hw`)

The game uses a second window `hw` for help, options, and inventory overlays
(`wclear(hw)`, `wmove(hw, …)`, `waddch/waddstr/wrefresh(hw)`).
In crossterm this would be an in-memory `Vec<Vec<char>>` buffer that is blitted
to the terminal with `MoveTo` + `print!` calls when ready to display.

### State save/restore of screen (`state.c`)

`state.c` iterates all cells with `mvwinch` (read) and `mvwaddch` (write) to
serialize and restore the visible screen.  With crossterm the equivalent is to
maintain a canonical `Vec<Vec<char>>` screen model as the source of truth and
serialize that directly.

### `inch()` / `mvinch()` — reading the screen back

Several places read what character is currently displayed on screen as game
state (e.g., `what is under the player?`).  This pattern requires maintaining
a separate virtual screen buffer alongside crossterm output so that those reads
can consult the model rather than the terminal.
