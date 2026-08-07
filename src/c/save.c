/*
 * save and restore routines
 *
 * @(#)save.c	4.33 (Berkeley) 06/01/83
 *
 * Rogue: Exploring the Dungeons of Doom
 * Copyright (C) 1980-1983, 1985, 1999 Michael Toy, Ken Arnold and Glenn Wichman
 * All rights reserved.
 *
 * See the file LICENSE.TXT for full copyright and licensing information.
 */

#include <stdlib.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <errno.h>
#include <signal.h>
#include <string.h>
#include <curses.h>
#include "rogue.h"
#include "score.h"

typedef struct stat STAT;

extern char version[];

/*
 * Helpers used by the Rust restore implementation.
 */
int
restore_link_invalid(char *file)
{
    STAT sbuf2;

    if (stat(file, &sbuf2) < 0)
        return 1;

    return (sbuf2.st_nlink != 1 || is_symlink(file));
}

int
restore_player_dead(void)
{
    return pstats.s_hpt <= 0;
}

