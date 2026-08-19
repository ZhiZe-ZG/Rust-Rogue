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

/*
 * new_level:
 *	Dig and draw a new level.
 *
 * Uses globals: player, hero, level, max_level, places, mlist,
 * lvl_obj, no_food, ntraps, stairs, seenstairs, rooms, passages.
 */
void
new_level()
{
    THING *tp;
    PLACE *pp;
    char *sp;
    int i;

    player.t_flags &= ~ISHELD;	/* unhold when you go down just in case */
    if (level > max_level)
	max_level = level;
    /*
     * Clean things off from last level
     */
    for (pp = places; pp < &places[MAXCOLS*MAXLINES]; pp++)
    {
	pp->p_ch = ' ';
	pp->p_flags = F_REAL;
	pp->p_monst = NULL;
    }
    clear();
    /*
     * Free up the monsters on the last level
     */
    for (tp = mlist; tp != NULL; tp = next(tp))
	free_list(tp->t_pack);
    free_list(mlist);
    /*
     * Throw away stuff left on the previous level (if anything)
     */
    free_list(lvl_obj);
    do_rooms();				/* Draw rooms */
    do_passages();			/* Draw passages */
    no_food++;
    put_things();			/* Place objects (if any) */
    /*
     * Place the traps
     */
    if (rnd(10) < level)
    {
	ntraps = rnd(level / 4) + 1;
	if (ntraps > MAXTRAPS)
	    ntraps = MAXTRAPS;
	i = ntraps;
	while (i--)
	{
	    /*
	     * not only wouldn't it be NICE to have traps in mazes
	     * (not that we care about being nice), since the trap
	     * number is stored where the passage number is, we
	     * can't actually do it.
	     */
	    do
	    {
		find_floor((struct room *) NULL, &stairs, FALSE, FALSE);
	    } while (chat(stairs.y, stairs.x) != FLOOR);
	    sp = &flat(stairs.y, stairs.x);
	    *sp &= ~F_REAL;
	    *sp |= rnd(NTRAPS);
	}
    }
    /*
     * Place the staircase down.
     */
    find_floor((struct room *) NULL, &stairs, FALSE, FALSE);
    chat(stairs.y, stairs.x) = STAIRS;
    seenstairs = FALSE;

    for (tp = mlist; tp != NULL; tp = next(tp))
	tp->t_room = roomin(&tp->t_pos);

    find_floor((struct room *) NULL, &hero, FALSE, TRUE);
    enter_room(&hero);
    mvaddch(hero.y, hero.x, PLAYER);
    if (on(player, SEEMONST))
	turn_see(FALSE);
    if (on(player, ISHALU))
	visuals();
}

/* put_things() is now implemented in Rust (src/rust/src/rooms/ffi.rs). */

