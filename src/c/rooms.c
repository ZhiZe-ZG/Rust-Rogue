/*
 * Create the layout for the new level
 *
 * @(#)rooms.c	4.45 (Berkeley) 02/05/99
 *
 * Rogue: Exploring the Dungeons of Doom
 * Copyright (C) 1980-1983, 1985, 1999 Michael Toy, Ken Arnold and Glenn Wichman
 * All rights reserved.
 *
 * See the file LICENSE.TXT for full copyright and licensing information.
 */

#include <ctype.h>
#include <curses.h>
#include "rogue.h"

#define GOLDGRP 1

/*
 * do_rooms:
 *	Create rooms and corridors with a connectivity graph
 */

void
do_rooms()
{
    int i;
    struct room *rp;
    THING *tp;
    int left_out;
    static coord top;
    coord bsze;				/* maximum room size */
    coord mp;

    bsze.x = NUMCOLS / 3;
    bsze.y = NUMLINES / 3;
    /*
     * Clear things for a new level
     */
    for (rp = rooms; rp < &rooms[MAXROOMS]; rp++)
    {
	rp->r_goldval = 0;
	rp->r_nexits = 0;
	rp->r_flags = 0;
    }
    /*
     * Put the gone rooms, if any, on the level
     */
    left_out = rnd(4);
    for (i = 0; i < left_out; i++)
	rooms[rnd_room()].r_flags |= ISGONE;
    /*
     * dig and populate all the rooms on the level
     */
    for (i = 0, rp = rooms; i < MAXROOMS; rp++, i++)
    {
	/*
	 * Find upper left corner of box that this room goes in
	 */
	top.x = (i % 3) * bsze.x + 1;
	top.y = (i / 3) * bsze.y;
	if (rp->r_flags & ISGONE)
	{
	    /*
	     * Place a gone room.  Make certain that there is a blank line
	     * for passage drawing.
	     */
	    do
	    {
		rp->r_pos.x = top.x + rnd(bsze.x - 2) + 1;
		rp->r_pos.y = top.y + rnd(bsze.y - 2) + 1;
		rp->r_max.x = -NUMCOLS;
		rp->r_max.y = -NUMLINES;
	    } until (rp->r_pos.y > 0 && rp->r_pos.y < NUMLINES-1);
	    continue;
	}
	/*
	 * set room type
	 */
	if (rnd(10) < level - 1)
	{
	    rp->r_flags |= ISDARK;		/* dark room */
	    if (rnd(15) == 0)
		rp->r_flags = ISMAZE;		/* maze room */
	}
	/*
	 * Find a place and size for a random room
	 */
	if (rp->r_flags & ISMAZE)
	{
	    rp->r_max.x = bsze.x - 1;
	    rp->r_max.y = bsze.y - 1;
	    if ((rp->r_pos.x = top.x) == 1)
		rp->r_pos.x = 0;
	    if ((rp->r_pos.y = top.y) == 0)
	    {
		rp->r_pos.y++;
		rp->r_max.y--;
	    }
	}
	else
	    do
	    {
		rp->r_max.x = rnd(bsze.x - 4) + 4;
		rp->r_max.y = rnd(bsze.y - 4) + 4;
		rp->r_pos.x = top.x + rnd(bsze.x - rp->r_max.x);
		rp->r_pos.y = top.y + rnd(bsze.y - rp->r_max.y);
	    } until (rp->r_pos.y != 0);
	draw_room(rp);
	/*
	 * Put the gold in
	 */
	if (rnd(2) == 0 && (!amulet || level >= max_level))
	{
	    THING *gold;

	    gold = new_item();
	    gold->o_goldval = rp->r_goldval = GOLDCALC;
	    find_floor(rp, &rp->r_gold, FALSE, FALSE);
	    gold->o_pos = rp->r_gold;
	    chat(rp->r_gold.y, rp->r_gold.x) = GOLD;
	    gold->o_flags = ISMANY;
	    gold->o_group = GOLDGRP;
	    gold->o_type = GOLD;
	    attach(lvl_obj, gold);
	}
	/*
	 * Put the monster in
	 */
	if (rnd(100) < (rp->r_goldval > 0 ? 80 : 25))
	{
	    tp = new_item();
	    find_floor(rp, &mp, FALSE, TRUE);
	    new_monster(tp, randmonster(FALSE), &mp);
	    give_pack(tp);
	}
    }
}

/*
 * rnd_pos:
 *	Pick a random spot in a room
 */

void
rnd_pos(struct room *rp, coord *cp)
{
    cp->x = rp->r_pos.x + rnd(rp->r_max.x - 2) + 1;
    cp->y = rp->r_pos.y + rnd(rp->r_max.y - 2) + 1;
}

/*
 * find_floor:
 *	Find a valid floor spot in this room.  If rp is NULL, then
 *	pick a new room each time around the loop.
 */
bool
find_floor(struct room *rp, coord *cp, int limit, bool monst)
{
    PLACE *pp;
    int cnt;
    char compchar = 0;
    bool pickroom;

    pickroom = (bool)(rp == NULL);

    if (!pickroom)
	compchar = ((rp->r_flags & ISMAZE) ? PASSAGE : FLOOR);
    cnt = limit;
    for (;;)
    {
	if (limit && cnt-- == 0)
	    return FALSE;
	if (pickroom)
	{
	    rp = &rooms[rnd_room()];
	    compchar = ((rp->r_flags & ISMAZE) ? PASSAGE : FLOOR);
	}
	rnd_pos(rp, cp);
	pp = INDEX(cp->y, cp->x);
	if (monst)
	{
	    if (pp->p_monst == NULL && step_ok(pp->p_ch))
		return TRUE;
	}
	else if (pp->p_ch == compchar)
	    return TRUE;
    }
}

