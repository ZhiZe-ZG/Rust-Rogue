/*
 * new_level:
 *	Dig and draw a new level
 *
 * @(#)new_level.c	4.38 (Berkeley) 02/05/99
 *
 * Rogue: Exploring the Dungeons of Doom
 * Copyright (C) 1980-1983, 1985, 1999 Michael Toy, Ken Arnold and Glenn Wichman
 * All rights reserved.
 *
 * See the file LICENSE.TXT for full copyright and licensing information.
 */

#include <curses.h>
#include <string.h>
#include "rogue.h"

#define TREAS_ROOM 20	/* one chance in TREAS_ROOM for a treasure room */
#define MAXTREAS 10	/* maximum number of treasures in a treasure room */
#define MINTREAS 2	/* minimum number of treasures in a treasure room */

/* new_level() is now implemented in Rust (src/rust/src/rooms/ffi.rs). */
/* put_things() is now implemented in Rust (src/rust/src/rooms/ffi.rs). */

