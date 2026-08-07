/*
 * hero movement commands
 *
 * @(#)move.c	4.49 (Berkeley) 02/05/99
 *
 * Rogue: Exploring the Dungeons of Doom
 * Copyright (C) 1980-1983, 1985, 1999 Michael Toy, Ken Arnold and Glenn Wichman
 * All rights reserved.
 *
 * See the file LICENSE.TXT for full copyright and licensing information.
 */

#include <curses.h>
#include <ctype.h>
#include "rogue.h"

/*
 * used to hold the new hero position
 */

coord nh;

/* do_run is implemented in Rust: src/rust/src/player.rs. */

/* do_move is implemented in Rust: src/rust/src/player.rs. */

/*
 * turn_ok:
 *	Decide whether it is legal to turn onto the given space
 */
bool
turn_ok(int y, int x)
{
    PLACE *pp;

    pp = INDEX(y, x);
    return (pp->p_ch == DOOR
	|| (pp->p_flags & (F_REAL|F_PASS)) == (F_REAL|F_PASS));
}

/*
 * turnref:
 *	Decide whether to refresh at a passage turning or not
 */

void
turnref()
{
    PLACE *pp;

    pp = INDEX(hero.y, hero.x);
    if (!(pp->p_flags & F_SEEN))
    {
	if (jump)
	{
	    leaveok(stdscr, TRUE);
	    refresh();
	    leaveok(stdscr, FALSE);
	}
	pp->p_flags |= F_SEEN;
    }
}

/*
 * door_open:
 *	Called to illuminate a room.  If it is dark, remove anything
 *	that might move.
 */

void
door_open(struct room *rp)
{
    int y, x;

    if (!(rp->r_flags & ISGONE))
	for (y = rp->r_pos.y; y < rp->r_pos.y + rp->r_max.y; y++)
	    for (x = rp->r_pos.x; x < rp->r_pos.x + rp->r_max.x; x++)
		if (isupper(winat(y, x)))
		    wake_monster(y, x);
}

/*
 * be_trapped:
 *	The guy stepped on a trap.... Make him pay.
 */
char
be_trapped(coord *tc)
{
    PLACE *pp;
    THING *arrow;
    char tr;

    if (on(player, ISLEVIT))
	return T_RUST;	/* anything that's not a door or teleport */
    running = FALSE;
    count = FALSE;
    pp = INDEX(tc->y, tc->x);
    pp->p_ch = TRAP;
    tr = pp->p_flags & F_TMASK;
    pp->p_flags |= F_SEEN;
    switch (tr)
    {
	case T_DOOR:
	    level++;
	    new_level();
	    msg("you fell into a trap!");
	when T_BEAR:
	    no_move += BEARTIME;
	    msg("you are caught in a bear trap");
        when T_MYST:
            switch(rnd(11))
            {
                case 0: msg("you are suddenly in a parallel dimension");
                when 1: msg("the light in here suddenly seems %s", rainbow[rnd(cNCOLORS)]);
                when 2: msg("you feel a sting in the side of your neck");
                when 3: msg("multi-colored lines swirl around you, then fade");
                when 4: msg("a %s light flashes in your eyes", rainbow[rnd(cNCOLORS)]);
                when 5: msg("a spike shoots past your ear!");
                when 6: msg("%s sparks dance across your armor", rainbow[rnd(cNCOLORS)]);
                when 7: msg("you suddenly feel very thirsty");
                when 8: msg("you feel time speed up suddenly");
                when 9: msg("time now seems to be going slower");
                when 10: msg("you pack turns %s!", rainbow[rnd(cNCOLORS)]);
            }
	when T_SLEEP:
	    no_command += SLEEPTIME;
	    player.t_flags &= ~ISRUN;
	    msg("a strange white mist envelops you and you fall asleep");
	when T_ARROW:
	    if (swing(pstats.s_lvl - 1, pstats.s_arm, 1))
	    {
		pstats.s_hpt -= roll(1, 6);
		if (pstats.s_hpt <= 0)
		{
		    msg("an arrow killed you");
		    death('a');
		}
		else
		    msg("oh no! An arrow shot you");
	    }
	    else
	    {
		arrow = new_item();
		init_weapon(arrow, ARROW);
		arrow->o_count = 1;
		arrow->o_pos = hero;
		fall(arrow, FALSE);
		msg("an arrow shoots past you");
	    }
	when T_TELEP:
	    /*
	     * since the hero's leaving, look() won't put a TRAP
	     * down for us, so we have to do it ourself
	     */
	    teleport();
	    mvaddch(tc->y, tc->x, TRAP);
	when T_DART:
	    if (!swing(pstats.s_lvl+1, pstats.s_arm, 1))
		msg("a small dart whizzes by your ear and vanishes");
	    else
	    {
		pstats.s_hpt -= roll(1, 4);
		if (pstats.s_hpt <= 0)
		{
		    msg("a poisoned dart killed you");
		    death('d');
		}
		if (!ISWEARING(R_SUSTSTR) && !save(VS_POISON))
		    chg_str(-1);
		msg("a small dart just hit you in the shoulder");
	    }
	when T_RUST:
	    msg("a gush of water hits you on the head");
	    rust_armor(cur_armor);
    }
    flush_type();
    return tr;
}

/*
 * rndmove:
 *	Move in a random direction if the monster/person is confused
 */
coord *
rndmove(THING *who)
{
    THING *obj;
    int x, y;
    char ch;
    static coord ret;  /* what we will be returning */

    y = ret.y = who->t_pos.y + rnd(3) - 1;
    x = ret.x = who->t_pos.x + rnd(3) - 1;
    /*
     * Now check to see if that's a legal move.  If not, don't move.
     * (I.e., bump into the wall or whatever)
     */
    if (y == who->t_pos.y && x == who->t_pos.x)
	return &ret;
    if (!diag_ok(&who->t_pos, &ret))
	goto bad;
    else
    {
	ch = winat(y, x);
	if (!step_ok(ch))
	    goto bad;
	if (ch == SCROLL)
	{
	    for (obj = lvl_obj; obj != NULL; obj = next(obj))
		if (y == obj->o_pos.y && x == obj->o_pos.x)
		    break;
	    if (obj != NULL && obj->o_which == S_SCARE)
		goto bad;
	}
    }
    return &ret;

bad:
    ret = who->t_pos;
    return &ret;
}

/* rust_armor is implemented in Rust: src/rust/src/armor.rs. */
